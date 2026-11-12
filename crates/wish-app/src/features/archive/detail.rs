//! A character's page: the wish art and the facts, what the player has (when
//! they own it), then one section after another, as genshinlist.vercel.app
//! lays a character out: the stats at the level on the slider and what it
//! costs to get there, the stats at each ascension, the talents with their
//! values at the talent level on the slider (and what a level costs), the
//! constellations.

use std::collections::BTreeMap;

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Div, FontWeight, ImageSource, InteractiveElement as _, IntoElement,
    ObjectFit, ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Window, div, img, linear_color_stop, linear_gradient, rems,
};
use wish_core::archive::{ArchiveData, CharacterPage, Talent};
use wish_core::model::Rarity;
use wish_core::planner::{Character, CharacterGoal, Level, MaterialId, PlannerData};

use super::CharactersView;
use crate::app::images::Images;
use wish_ui::{MaterialTile, PageFrame, badge, button, card, card_title, format, rich_text, theme};

impl CharactersView {
    pub(super) fn render_character(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_controls(key, window, cx);
        let owned = self.owned_character(key, cx);
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images().clone(),
        );
        let back = button("back-to-characters")
            .ghost()
            .icon(IconName::ArrowLeft)
            .label("All characters")
            .on_click(cx.listener(|this, _, _, cx| this.show_list(cx)));
        let (Some(character), Some(page)) = (planner.character(key), archive.character(key)) else {
            return PageFrame::new(
                "Character",
                div()
                    .px_6()
                    .child("This character is not in the game data."),
            )
            .action(back)
            .into_any_element();
        };

        // One page of sections, as genshinlist.vercel.app lays a character
        // out: what the player has, the stats at a level and their cost, the
        // stats at each ascension, the talents, the constellations.
        let width = crate::shell::content_width(window);
        let mine = owned
            .as_ref()
            .map(|owned| self.my_character(owned, &character.name, cx));
        let stats = self.profile(character, page, &planner, &archive, &images, cx);
        let ascension = ascension_table(page, &archive, cx);
        let talents = self.talents(character, page, &planner, &images, cx);
        let constellations = constellations(page, &images, cx);

        let body = div()
            .id("character-page")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_6()
                    .px_6()
                    .pt_2()
                    .pb_8()
                    .child(hero(character, page, &images, width, window, cx))
                    .children(mine)
                    .child(section("Stats", stats, cx))
                    .child(section("Ascension", ascension, cx))
                    .child(section("Talents", talents, cx))
                    .child(section("Constellations", constellations, cx)),
            );
        let plan = button("plan-character")
            .primary()
            .icon(IconName::Plus)
            .label("Add to the planner")
            .on_click(cx.listener({
                let (key, name) = (character.key.clone(), character.name.clone());
                move |this, _, window, cx| this.add_to_plan(key.clone(), name.clone(), window, cx)
            }));
        let mut frame = PageFrame::new(character.name.clone(), body).action(plan);
        if owned.is_none() {
            frame = frame.action(
                button("own-character")
                    .icon(IconName::Plus)
                    .label("Add to my characters")
                    .on_click(cx.listener({
                        let (key, name) = (character.key.clone(), character.name.clone());
                        move |this, _, window, cx| this.own(key.clone(), name.clone(), window, cx)
                    })),
            );
        }
        let mut frame = frame.action(back);
        if let Some(title) = &page.title {
            frame = frame.subtitle(title.clone());
        }
        frame.body_scrolls().into_any_element()
    }

    fn profile(
        &self,
        character: &Character,
        page: &CharacterPage,
        planner: &PlannerData,
        archive: &ArchiveData,
        images: &Images,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let level = self.level(cx);
        let stats = archive.stats(page, level);
        let special = if page.stats.special == "Elemental Mastery" {
            format::count(stats.special.round() as u64)
        } else {
            format!("{:.1}%", stats.special * 100.0)
        };
        let rows = [
            ("Base HP", format::count(stats.hp.round() as u64)),
            ("Base ATK", format::count(stats.attack.round() as u64)),
            ("Base DEF", format::count(stats.defense.round() as u64)),
            (page.stats.special.as_str(), special),
        ];
        let level_label = format!("Lv. {}/{}", level.level, level.cap());
        let stats_card = card(cx)
            .p_4()
            .gap_3()
            .child(card_title("Base stats", cx).child(level_badge(&level_label, cx)))
            .child(
                Slider::new(&self.level)
                    .cursor_pointer()
                    .accessibility_label("Level"),
            )
            .child(table(rows, cx));

        let need = planner.character_cost(
            character,
            &CharacterGoal {
                from: Level::START,
                to: level,
                talents: [(1, 1); 3],
            },
        );
        let cost_card = card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title(format!("To reach {level_label}"), cx).child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("from level 1"),
                ),
            )
            .child(materials(&need.materials_with_exp(), planner, images, cx));

        div()
            .grid()
            .grid_cols(2)
            .gap_4()
            .items_start()
            .child(stats_card)
            .child(cost_card)
            .into_any_element()
    }

    fn talents(
        &self,
        character: &Character,
        page: &CharacterPage,
        planner: &PlannerData,
        images: &Images,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let level = self.talent_level(cx);
        // Levels past 10 come from constellations; materials stop at 10.
        let to = level.min(10);
        let need = planner.character_cost(
            character,
            &CharacterGoal {
                from: Level::START,
                to: Level::START,
                talents: [(1, to), (1, 1), (1, 1)],
            },
        );
        let level_card = card(cx)
            .p_4()
            .gap_3()
            .child(card_title("Talent level", cx).child(level_badge(&format!("Lv. {level}"), cx)))
            .child(
                Slider::new(&self.talent_level)
                    .cursor_pointer()
                    .accessibility_label("Talent level"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(if level > 10 {
                        "Levels 11 to 15 come from constellations; the materials stop at 10."
                            .to_owned()
                    } else {
                        format!("Raising one talent from 1 to {level}:")
                    }),
            )
            .when(to > 1, |this| {
                this.child(materials(&need.materials_with_exp(), planner, images, cx))
            });

        div()
            .v_flex()
            .gap_4()
            .child(level_card)
            .children(
                page.talents
                    .iter()
                    .map(|talent| talent_card(talent, level, images, cx)),
            )
            .into_any_element()
    }
}

/// A section of a character's page: its title after a gold diamond, then
/// its content.
fn section(title: &str, content: AnyElement, cx: &App) -> Div {
    let gold = theme::palette(cx).gold;
    div()
        .v_flex()
        .gap_3()
        .child(
            div()
                .h_flex()
                .gap_2()
                .child(Icon::new(IconName::Diamond).size_4().text_color(gold))
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .child(title.to_owned()),
                ),
        )
        .child(content)
}

/// The stats at each bound of each ascension phase, as the game writes the
/// level: "20/20" before ascending, "20/40" after.
fn ascension_table(page: &CharacterPage, archive: &ArchiveData, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let bounds = [
        (1, 0),
        (20, 0),
        (20, 1),
        (40, 1),
        (40, 2),
        (50, 2),
        (50, 3),
        (60, 3),
        (60, 4),
        (70, 4),
        (70, 5),
        (80, 5),
        (80, 6),
        (90, 6),
    ];
    let special_name = page.stats.special.clone();
    let percent = special_name != "Elemental Mastery";
    let row = |cells: [String; 5], head: bool, even: bool| {
        div()
            .h_flex()
            .px_4()
            .py_2()
            .when(even, |this| this.bg(theme.table_even))
            .when(head, |this| {
                this.text_sm()
                    .text_color(theme.muted_foreground)
                    .border_b_1()
                    .border_color(theme.table_row_border)
            })
            .children(cells.into_iter().enumerate().map(|(ix, cell)| {
                let cell = div().child(cell);
                if ix == 0 {
                    cell.w(rems(6.)).font_weight(FontWeight::SEMIBOLD)
                } else {
                    cell.flex_1().text_right()
                }
            }))
    };
    let header = row(
        [
            "Level".to_owned(),
            "Base HP".to_owned(),
            "Base ATK".to_owned(),
            "Base DEF".to_owned(),
            special_name,
        ],
        true,
        false,
    );
    let rows = bounds
        .iter()
        .enumerate()
        .filter_map(|(ix, (level, phase))| {
            let level = Level::new(*level, *phase)?;
            let stats = archive.stats(page, level);
            let special = if percent {
                format!("{:.1}%", stats.special * 100.0)
            } else {
                format::count(stats.special.round() as u64)
            };
            Some(row(
                [
                    format!("{}/{}", level.level, level.cap()),
                    format::count(stats.hp.round() as u64),
                    format::count(stats.attack.round() as u64),
                    format::count(stats.defense.round() as u64),
                    special,
                ],
                false,
                ix % 2 == 1,
            ))
        });
    card(cx)
        .py_2()
        .overflow_hidden()
        .child(header)
        .children(rows)
        .into_any_element()
}

/// The hero's height, in rems.
const HERO_REMS: f32 = 22.;

/// The page's head: the wish art large on the right, over a blurred copy of
/// itself, the element, the stars, the description and the facts on the left.
fn hero(
    character: &Character,
    page: &CharacterPage,
    images: &Images,
    width: Pixels,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let rarity = if character.rarity == 5 {
        Rarity::Five
    } else {
        Rarity::Four
    };
    let facts = facts_grid(
        &[
            ("Element", Some(character.element.as_str())),
            ("Weapon", Some(character.weapon.as_str())),
            ("Region", page.region.as_deref()),
            ("Affiliation", page.affiliation.as_deref()),
            ("Birthday", page.birthday.as_deref()),
            ("Constellation", page.constellation.as_deref()),
            ("Released in", page.version.as_deref()),
        ],
        cx,
    );
    let info = div()
        .child(
            div()
                .h_flex()
                .gap_3()
                .children(
                    images
                        .element(&character.element)
                        .map(|icon| img(icon).size_9().flex_shrink_0()),
                )
                .child(
                    div()
                        .text_2xl()
                        .text_color(theme::rarity(rarity, cx))
                        .child("★".repeat(usize::from(character.rarity))),
                ),
        )
        .children(
            page.description
                .as_deref()
                .map(|text| div().text_lg().line_clamp(3).child(rich_text(text))),
        )
        .child(facts);
    // The art is twice as wide as tall, its character in the middle third:
    // drawn larger than the banner, which crops it.
    hero_banner(
        info,
        images
            .splash(page.splash.as_deref())
            .map(|art| (art, 2., 1.3)),
        images.backdrop(page.splash.as_deref()),
        width,
        window,
        cx,
    )
}

/// A page's banner: `info` on the left; on the right `art` (with its width
/// as a share of its height, and how many times the banner's height it is
/// drawn, cropped by the banner), centred three quarters of the way across;
/// under both `backdrop`, a blurred copy of the art faded into the card under
/// the text. `width` is the page's.
pub(super) fn hero_banner(
    info: Div,
    art: Option<(ImageSource, f32, f32)>,
    backdrop: Option<ImageSource>,
    width: Pixels,
    window: &Window,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let height = window.rem_size() * HERO_REMS;
    // The art beside the text when there is room for both.
    let beside = width > window.rem_size() * 52.;
    let surface = theme.group_box;
    let layer = || div().absolute().top_0().left_0().w(width).h(height);
    // Clipping is rectangular: each layer rounds its own corners.
    let backdrop = backdrop.map(|image| {
        layer()
            .rounded(theme.radius_lg)
            .overflow_hidden()
            .opacity(0.85)
            .child(
                img(image)
                    .w(width)
                    .h(height)
                    .aspect_ratio(width / height)
                    .object_fit(ObjectFit::Cover),
            )
    });
    // Solid under the text, clear under the art.
    let fade = layer().rounded(theme.radius_lg).bg(linear_gradient(
        90.,
        linear_color_stop(surface, 0.2),
        linear_color_stop(surface.opacity(0.), if beside { 0.75 } else { 1.4 }),
    ));
    let art = art.filter(|_| beside).map(|(art, ratio, scale)| {
        let art_height = (height * scale).min(width * 0.9 / ratio);
        let art_width = art_height * ratio;
        div()
            .absolute()
            .left(width * 0.76 - art_width / 2.)
            .top((height - art_height) / 2.)
            .child(
                img(art)
                    .w(art_width)
                    .h(art_height)
                    .aspect_ratio(ratio)
                    .object_fit(ObjectFit::Contain),
            )
    });
    div()
        .relative()
        .w(width)
        .h(height)
        .rounded(theme.radius_lg)
        .overflow_hidden()
        .bg(surface)
        .when(!theme.is_dark(), |this| {
            this.border_1().border_color(theme.border)
        })
        .children(backdrop)
        .child(fade)
        .children(art)
        .child(
            info.relative()
                .v_flex()
                .justify_center()
                .gap_4()
                .h(height)
                .w(if beside { width * 0.55 } else { width })
                .p_6(),
        )
}

/// Facts in two columns of label and value; those without a value left out.
pub(super) fn facts_grid(facts: &[(&str, Option<&str>)], cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .grid()
        .grid_cols(2)
        .gap_x_6()
        .gap_y_1()
        .text_sm()
        .children(facts.iter().filter_map(|&(label, value)| {
            let value = value?;
            Some(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        div()
                            .w(rems(6.5))
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground)
                            .child(label.to_owned()),
                    )
                    .child(div().truncate().child(value.to_owned())),
            )
        }))
}

/// A small primary badge for the level a slider is at.
pub(super) fn level_badge(text: &str, cx: &App) -> Div {
    let theme = cx.theme();
    badge(
        text.to_owned(),
        (theme.primary, theme.primary.opacity(0.16)),
        cx,
    )
    .text_sm()
}

/// Two columns: a muted label, the value on the right.
pub(super) fn table<L: Into<SharedString>>(
    rows: impl IntoIterator<Item = (L, String)>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    div()
        .v_flex()
        .children(rows.into_iter().map(|(label, value)| {
            div()
                .h_flex()
                .justify_between()
                .gap_4()
                .py_1p5()
                .border_t_1()
                .border_color(theme.border)
                .child(div().text_color(theme.muted_foreground).child(label.into()))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
        }))
}

/// The materials of a cost as the game shows them, in its inventory order.
pub(super) fn materials(
    need: &BTreeMap<MaterialId, u64>,
    planner: &PlannerData,
    images: &Images,
    cx: &App,
) -> AnyElement {
    let mut items: Vec<_> = need
        .iter()
        .filter_map(|(&id, &count)| Some((planner.material(id)?, count)))
        .collect();
    if items.is_empty() {
        return div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("Nothing to spend.")
            .into_any_element();
    }
    items.sort_by_key(|(material, _)| (material.sort, material.id));
    div()
        .h_flex()
        .flex_wrap()
        .gap_2()
        .children(items.into_iter().map(|(material, count)| {
            div()
                .id(SharedString::from(format!("material-{}", material.id)))
                .child(MaterialTile::new(
                    images.material(material),
                    material.rarity,
                    format::compact(count),
                ))
                .tooltip({
                    let text =
                        SharedString::from(format!("{} × {}", format::count(count), material.name));
                    move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(text.clone()).build(window, cx)
                    }
                })
        }))
        .into_any_element()
}

/// The dark round tile the game draws its white talent and constellation
/// glyphs on.
fn glyph(icon: Option<ImageSource>, cx: &App) -> Div {
    let theme = cx.theme();
    let tile = if theme.is_dark() {
        theme.sidebar
    } else {
        theme.foreground
    };
    div()
        .size_12()
        .flex_shrink_0()
        .rounded_full()
        .bg(tile)
        .p_1()
        .children(icon.map(|icon| img(icon).size_full()))
}

fn talent_card(talent: &Talent, level: u8, images: &Images, cx: &App) -> Div {
    let theme = cx.theme();
    let icon = talent.icon.as_deref().and_then(|file| images.skill(file));
    let values = talent.has_levels().then(|| table(talent.values(level), cx));
    card(cx).p_4().child(
        div()
            .h_flex()
            .items_start()
            .gap_4()
            .child(glyph(icon, cx))
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(talent.kind.clone()),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(talent.name.clone()),
                            ),
                    )
                    .child(div().text_sm().child(rich_text(&talent.description)))
                    .children(values.map(|values| div().pt_1().child(values))),
            ),
    )
}

fn constellations(page: &CharacterPage, images: &Images, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .v_flex()
        .gap_4()
        .children(
            page.constellations
                .iter()
                .enumerate()
                .map(|(i, constellation)| {
                    let icon = constellation
                        .icon
                        .as_deref()
                        .and_then(|file| images.skill(file));
                    card(cx).p_4().child(
                        div()
                            .h_flex()
                            .items_start()
                            .gap_4()
                            .child(glyph(icon, cx))
                            .child(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_2()
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(badge(
                                                format!("C{}", i + 1),
                                                (theme.primary, theme.primary.opacity(0.16)),
                                                cx,
                                            ))
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(constellation.name.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .child(rich_text(&constellation.description)),
                                    ),
                            ),
                    )
                }),
        )
        .into_any_element()
}

impl CharactersView {
    /// Adds the character to the ascension planner, from where the player
    /// has it (or level 1) to 90 with talents to 10, and says so.
    fn add_to_plan(
        &mut self,
        key: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let account = self.session.read(cx).account();
        let goal = match self.owned_character(&key, cx) {
            Some(owned) => CharacterGoal {
                from: owned.level,
                to: Level::MAX,
                talents: owned.talents.map(|t| (t, t.max(10))),
            },
            None => CharacterGoal::FULL,
        };
        let target = wish_core::planner::PlanTarget::Character { key, goal };
        let task = self.library.update(cx, |library, cx| {
            library.add_plan_entry(account, target, cx)
        });
        let from = goal.from;
        cx.spawn_in(window, async move |_, cx| {
            let message = match task.await {
                Ok(_) => crate::app::notices::Notice::done(format!(
                    "{name} is in the planner, from level {} to 90 with talents to 10.",
                    from.level
                )),
                Err(err) => crate::app::notices::Notice::failed(format!(
                    "{name} was not added to the planner: {err:#}"
                )),
            };
            cx.update(|_, cx| crate::app::notices::notify(message, cx))
                .ok();
        })
        .detach();
    }
}

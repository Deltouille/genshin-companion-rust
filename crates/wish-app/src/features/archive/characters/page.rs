//! A character's page: the wish art and the facts, what the player has (when
//! they own it), then one section after another, as genshinlist.vercel.app
//! lays a character out: the stats at the level on the slider and what it
//! costs to get there, the stats at each ascension, the talents with their
//! values at the talent level on the slider (and what a level costs), the
//! constellations.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::Slider;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Div, FontWeight, ImageSource, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Styled as _, Window, div, img, rems,
};
use wish_core::archive::{ArchiveData, CharacterPage, Talent};
use wish_core::model::Rarity;
use wish_core::planner::{Character, PlannerData};

use super::super::shared::{facts_grid, hero_banner, level_badge, materials, table};
use super::CharactersView;
use super::model;
use crate::app::images::Images;
use wish_ui::{PageFrame, badge, button, card, card_title, rich_text, theme};

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
            library.images(cx).clone(),
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
                move |this, _, _, cx| this.add_to_plan(key.clone(), name.clone(), cx)
            }));
        let mut frame = PageFrame::new(character.name.clone(), body).action(plan);
        if owned.is_none() {
            frame = frame.action(
                button("own-character")
                    .icon(IconName::Plus)
                    .label("Add to my characters")
                    .on_click(cx.listener({
                        let (key, name) = (character.key.clone(), character.name.clone());
                        move |this, _, _, cx| this.own(key.clone(), name.clone(), cx)
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
        let at = self.level.read(cx).value().end().round() as usize;
        let stats = model::stats(planner, archive, character, page, at);
        let stats_card = card(cx)
            .p_4()
            .gap_3()
            .child(card_title("Base stats", cx).child(level_badge(&stats.level_label, cx)))
            .child(
                Slider::new(&self.level)
                    .cursor_pointer()
                    .accessibility_label("Level"),
            )
            .child(table(stats.rows.clone(), cx));

        let cost_card = card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title(format!("To reach {}", stats.level_label), cx).child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("from level 1"),
                ),
            )
            .child(materials(&stats.cost, planner, images, cx));

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
        let talent = model::talent_cost(planner, character, self.talent_level(cx));
        let level = talent.level;
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
                    .child(talent.note.clone()),
            )
            .when_some(talent.cost.as_ref(), |this, cost| {
                this.child(materials(cost, planner, images, cx))
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

/// A section of a character's page: its title, then its content.
fn section(title: &str, content: AnyElement, _: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::BOLD)
                .child(title.to_owned()),
        )
        .child(content)
}

/// The stats at each bound of each ascension phase, as the game writes the
/// level: "20/20" before ascending, "20/40" after.
fn ascension_table(page: &CharacterPage, archive: &ArchiveData, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let special_name = page.stats.special.clone();
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
    let rows = (model::ascension(archive, page).into_iter())
        .enumerate()
        .map(|(ix, cells)| row(cells, false, ix % 2 == 1));
    card(cx)
        .py_2()
        .overflow_hidden()
        .child(header)
        .children(rows)
        .into_any_element()
}

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
    fn add_to_plan(&mut self, key: String, name: String, cx: &mut Context<Self>) {
        let account = self.session.read(cx).account();
        let goal = self.library.read(cx).owned(account).character_goal(&key);
        let target = wish_core::planner::PlanTarget::Character { key, goal };
        let task = self.library.update(cx, |library, cx| {
            library.add_plan_entry(account, target, cx)
        });
        let from = goal.from;
        crate::app::notices::report_outcome(
            task,
            format!("{name} was not added to the planner"),
            move |_| {
                format!(
                    "{name} is in the planner, from level {} to 90 with talents to 10.",
                    from.level
                )
            },
            cx,
        );
    }
}

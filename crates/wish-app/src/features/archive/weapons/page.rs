//! A weapon's page: the hero banner (its upright wish art beside the stars,
//! the description and the facts), the account's copies, the stats at the
//! level slider and what reaching it costs, the passive at a refinement. It
//! draws the [`WeaponPage`](super::model::WeaponPage).

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::Slider;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    Window, div,
};
use wish_ui::{PageFrame, Segmented, button, card, card_title, theme};

use super::super::shared::{facts_grid, hero_banner, level_badge, materials, table};
use super::WeaponsView;
use super::model;

impl WeaponsView {
    pub(super) fn render_weapon(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let library = self.library.read(cx);
        let (planner, archive, images) = (
            library.planner().clone(),
            library.archive().clone(),
            library.images(cx).clone(),
        );
        let back = button("back-to-weapons")
            .ghost()
            .icon(IconName::ArrowLeft)
            .label("All weapons")
            .on_click(cx.listener(|this, _, _, cx| this.show_list(cx)));
        let level = self.level.read(cx).value().end().round() as usize;
        let (Some(weapon), Some(page)) = (
            planner.weapon(key),
            model::page(&planner, &archive, key, level, self.refinement),
        ) else {
            return PageFrame::new(
                "Weapon",
                div().px_6().child("This weapon is not in the game data."),
            )
            .action(back)
            .into_any_element();
        };
        // What the account has of it, under the banner.
        let copies = self.my_copies(weapon, page.refinable, cx);
        let theme = cx.theme();
        let width = crate::shell::content_width(window);

        // The banner: the stars, the description and the facts beside the
        // wish art, which stands upright (half as wide as tall).
        let facts: Vec<(&str, Option<&str>)> = (page.facts.iter())
            .map(|(label, value)| (*label, value.as_deref()))
            .collect();
        let info = div()
            .child(
                div()
                    .text_2xl()
                    .text_color(theme::stars_color(page.rarity, cx))
                    .child("★".repeat(usize::from(page.rarity))),
            )
            .children(
                (page.description.clone()).map(|text| div().text_lg().line_clamp(3).child(text)),
            )
            .child(facts_grid(&facts, cx));
        let hero = hero_banner(
            info,
            images
                .splash(page.art.as_deref())
                .map(|art| (art, 0.5, 1.15)),
            images.backdrop(page.art.as_deref()),
            width,
            window,
            cx,
        );

        let stats_card = card(cx)
            .p_4()
            .gap_3()
            .child(card_title("Stats", cx).child(level_badge(&page.level_label, cx)))
            .child(
                Slider::new(&self.level)
                    .cursor_pointer()
                    .accessibility_label("Level"),
            )
            .child(table(page.stats.clone(), cx));
        let cost_card = card(cx)
            .p_4()
            .gap_3()
            .child(
                card_title(format!("To reach {}", page.level_label), cx).child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("from level 1"),
                ),
            )
            .child(materials(&page.cost, &planner, &images, cx));

        let passive = page.passive.as_ref().map(|passive| {
            let levels = (passive.refinements > 1).then(|| {
                (1..=passive.refinements)
                    .fold(Segmented::new("refinement"), |tabs, r| {
                        tabs.option(format!("R{r}"))
                    })
                    .selected(passive.refinement - 1)
                    .on_select(cx.listener(|this, ix: &usize, _, cx| {
                        this.refinement = ix + 1;
                        cx.notify();
                    }))
            });
            card(cx)
                .p_4()
                .gap_3()
                .child(card_title(passive.name.clone(), cx).children(levels))
                .child(
                    div()
                        .text_color(theme.foreground)
                        .child(passive.text.clone()),
                )
        });

        let body = div()
            .id("weapon-page")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_6()
                    .pt_2()
                    .pb_6()
                    .child(hero)
                    .child(copies)
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_4()
                            .items_start()
                            .child(stats_card)
                            .child(cost_card),
                    )
                    .children(passive),
            );
        let plan = button("plan-weapon")
            .primary()
            .icon(IconName::Plus)
            .label("Add to the planner")
            .on_click(cx.listener({
                let (key, name, rarity) = (weapon.key.clone(), weapon.name.clone(), weapon.rarity);
                move |this, _, _, cx| this.add_to_plan(key.clone(), name.clone(), rarity, cx)
            }));
        PageFrame::new(page.name.clone(), body)
            .subtitle(page.kind.clone())
            .action(plan)
            .action(back)
            .body_scrolls()
            .into_any_element()
    }
}

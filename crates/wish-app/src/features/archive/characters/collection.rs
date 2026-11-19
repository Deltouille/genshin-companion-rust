//! What the selected account owns, on the character pages: adding and
//! removing a character, marking a favourite, and the "My character" panel
//! (level, constellation, talents). Every change saves at once; a removal
//! asks first, since it forgets what was entered.

use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::{
    AnyElement, App, Context, Div, FontWeight, IntoElement, ParentElement as _, Styled as _,
    Window, div, rems,
};
use wish_core::collection::{MAX_CONSTELLATION, MAX_TALENT, OwnedCharacter};
use wish_core::model::AccountId;
use wish_core::planner::Level;

use super::CharactersView;
use crate::app::notices::{report, report_outcome};
use gpui_kit::prelude::FluentBuilder as _;
use wish_ui::{
    ButtonExt as _, Segmented, button, card, card_title, confirm_destructive, icon_button, picker,
};

impl CharactersView {
    /// The account whose characters are shown: the sidebar's.
    pub(super) fn account(&self, cx: &App) -> Option<AccountId> {
        self.session.read(cx).account()
    }

    /// What the account entered for character `key`, if it owns it.
    pub(super) fn owned_character(&self, key: &str, cx: &App) -> Option<OwnedCharacter> {
        let library = self.library.read(cx);
        library.owned(self.account(cx)).character(key).cloned()
    }

    /// Adds `key` to the account's characters, at level 1.
    pub(super) fn own(&mut self, key: String, name: String, cx: &mut App) {
        let account = self.account(cx);
        let task = self.library.update(cx, |library, cx| {
            library.save_owned_character(account, OwnedCharacter::new(key), cx)
        });
        report_outcome(
            task,
            t!("archive-not-added", name = name.as_str()),
            move |()| t!("archive-character-owned", name = name.as_str()),
            cx,
        );
    }

    /// Saves what changed on an owned character.
    pub(super) fn save_owned(&mut self, character: OwnedCharacter, cx: &mut App) {
        let account = self.account(cx);
        let task = self.library.update(cx, |library, cx| {
            library.save_owned_character(account, character, cx)
        });
        report_characters(task, cx);
    }

    /// Marks or unmarks `key` as a favourite.
    pub(super) fn toggle_favourite(&mut self, key: &str, cx: &mut App) {
        if let Some(mut character) = self.owned_character(key, cx) {
            character.favourite = !character.favourite;
            self.save_owned(character, cx);
        }
    }

    /// The level picked on the open character's page.
    pub(super) fn set_owned_level(&mut self, level: Level, cx: &mut Context<Self>) {
        let Some(key) = self.open.clone() else { return };
        if let Some(mut character) = self.owned_character(&key, cx)
            && character.level != level
        {
            character.level = level;
            self.synced = Some((key, Some(level)));
            self.save_owned(character, cx);
        }
    }

    /// Asks before forgetting that the account owns `key`.
    pub(super) fn confirm_disown(
        &mut self,
        key: String,
        name: String,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(account) = self.account(cx) else {
            return;
        };
        let library = self.library.downgrade();
        confirm_destructive(
            window,
            cx,
            t!("archive-remove-character-title", name = name.as_str()),
            t!("archive-remove-character-help"),
            t!("archive-remove"),
            move |_, cx| {
                let key = key.clone();
                let task = library.update(cx, |library, cx| {
                    library.remove_owned_character(account, key, cx)
                });
                if let Ok(task) = task {
                    report_characters(task, cx);
                }
            },
        );
    }

    /// Sets the page's controls from what the account owns when the page
    /// opens or the owned level changes: the level picker, and the sliders
    /// at the level the player has.
    pub(super) fn sync_controls(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let owned = self.owned_character(key, cx);
        let current = (key.to_owned(), owned.as_ref().map(|c| c.level));
        if self.synced.as_ref() == Some(&current) {
            return;
        }
        let first_time = self.synced.as_ref().is_none_or(|(open, _)| open != key);
        self.synced = Some(current);
        let Some(owned) = owned else { return };
        let levels = Level::steps();
        let index = levels.iter().position(|l| *l == owned.level).unwrap_or(0);
        self.owned_level.update(cx, |select, cx| {
            select.set_selected_value(&index, cx);
        });
        if first_time {
            self.level
                .update(cx, |slider, cx| slider.set_value(index as f32, window, cx));
            let skill = f32::from(owned.talents[1]);
            self.talent_level
                .update(cx, |slider, cx| slider.set_value(skill, window, cx));
        }
    }

    /// The panel with what the player has: level, constellation, talents,
    /// favourite, and the way to remove the character.
    pub(super) fn my_character(
        &self,
        character: &OwnedCharacter,
        name: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let gold = wish_ui::theme::rarity(wish_core::model::Rarity::Five, cx);
        let favourite = {
            let key = character.key.clone();
            let (icon, label) = if character.favourite {
                (IconName::StarFill, t!("archive-remove-favourite"))
            } else {
                (IconName::Star, t!("archive-add-favourite"))
            };
            icon_button("favourite", icon, label)
                .ghost()
                .when(character.favourite, |this| this.text_color(gold))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_favourite(&key, cx);
                }))
        };
        let remove = button("disown")
            .ghost()
            .label(t!("archive-remove"))
            .on_click(cx.listener({
                let (key, name) = (character.key.clone(), name.to_owned());
                move |this, _, window, cx| {
                    this.confirm_disown(key.clone(), name.clone(), window, cx);
                }
            }));

        let constellation = {
            let owned = character.clone();
            (0..=MAX_CONSTELLATION)
                .fold(Segmented::new("constellation"), |seg, c| {
                    seg.option(format!("C{c}"))
                })
                .selected(usize::from(character.constellation))
                .on_select(cx.listener(move |this, ix: &usize, _, cx| {
                    let mut owned = owned.clone();
                    owned.constellation = *ix as u8;
                    this.save_owned(owned, cx);
                }))
        };
        let names = crate::features::planner::talent_names();
        let mut talents = Vec::with_capacity(names.len());
        for (ix, talent) in names.iter().enumerate() {
            let level = character.talents[ix];
            let step = |delta: i8, icon: IconName, label: String, cx: &mut Context<Self>| {
                let mut owned = character.clone();
                let to = level.saturating_add_signed(delta).clamp(1, MAX_TALENT);
                owned.talents[ix] = to;
                icon_button(format!("talent-{ix}-{delta}"), icon, label)
                    .ghost()
                    .size_9()
                    .unavailable(to == level)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.save_owned(owned.clone(), cx);
                    }))
            };
            let (lower, raise) = (
                step(
                    -1,
                    IconName::Minus,
                    t!("archive-lower-talent", talent = talent.as_str()),
                    cx,
                ),
                step(
                    1,
                    IconName::Plus,
                    t!("archive-raise-talent", talent = talent.as_str()),
                    cx,
                ),
            );
            talents.push(
                div()
                    .v_flex()
                    .gap_1()
                    .child(div().text_sm().text_color(muted).child(talent.clone()))
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .child(lower)
                            .child(
                                div()
                                    .w(rems(2.))
                                    .text_center()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(level.to_string()),
                            )
                            .child(raise),
                    ),
            );
        }

        // The copy it holds, set on the weapon's page.
        let held_weapon = {
            let library = self.library.read(cx);
            let held = library.owned(self.account(cx)).weapon_of(&character.key);
            let text = match held {
                Some(copy) => {
                    let name = library
                        .planner()
                        .weapon(&copy.key)
                        .map_or(copy.key.as_str(), |w| w.name.as_str());
                    t!(
                        "archive-held-weapon",
                        name = name,
                        refinement = copy.refinement,
                        level = copy.level.level,
                        cap = copy.level.cap()
                    )
                }
                None => t!("archive-no-weapon-held"),
            };
            div().h(rems(2.5)).flex().items_center().child(text)
        };
        card(cx)
            .p_4()
            .gap_4()
            .child(
                card_title(t!("archive-my-character"), cx)
                    .child(div().h_flex().gap_2().child(favourite).child(remove)),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap_x_8()
                    .gap_y_4()
                    .child(field(
                        &t!("archive-level"),
                        div().w(rems(9.)).child(
                            picker("owned-level", &self.owned_level)
                                .accessibility_label(t!("archive-level"))
                                .menu_width(rems(9.)),
                        ),
                        cx,
                    ))
                    .child(field(&t!("archive-constellation"), constellation, cx))
                    .child(div().h_flex().gap_6().children(talents))
                    .child(field(&t!("archive-fact-weapon"), held_weapon, cx)),
            )
            .into_any_element()
    }
}

/// A control with its label above.
fn field(label: &str, control: impl IntoElement, cx: &App) -> Div {
    div()
        .v_flex()
        .gap_1()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_owned()),
        )
        .child(control)
}

/// Shows a notice if a write of the collection fails.
pub(super) fn report_characters<T: 'static>(task: gpui_kit::Task<anyhow::Result<T>>, cx: &mut App) {
    report(task, t!("archive-characters-not-saved"), cx);
}

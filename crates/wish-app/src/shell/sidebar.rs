//! The sidebar: the app's name, the account picker, the Overview, one group
//! per feature (the wish counter: one page per banner type; the archive: the
//! characters and weapons; the planner), then Settings and Credits.
//!
//! Its own column rather than GPUI Kit's `Sidebar`, whose items have a fixed
//! 28 px height: the entries are Material 3's 56 px [`NavItem`]s.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName};
use gpui_kit::{
    Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, div, prelude::FluentBuilder as _,
};
use wish_core::model::AccountId;
use wish_ui::{NavGroup, NavItem};
use wish_ui::{PickerItem, picker};

use super::{ArchivePage, Feature, Page, Shell, WishPage};
use crate::APP_TITLE;
use crate::app::library::AccountSummary;

impl Shell {
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_accounts = !self.library.read(cx).accounts().is_empty();
        let item =
            |id: &str, label: SharedString, icon: Icon, page: Page, cx: &mut Context<Self>| {
                NavItem::new(SharedString::from(format!("nav-{id}")), label, icon)
                    .active(self.page == page)
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
            };
        // A page of a feature, listed in its group.
        let nested = |id: &str, label: SharedString, page: Page, cx: &mut Context<Self>| {
            NavItem::nested(SharedString::from(format!("nav-{id}")), label)
                .active(self.page == page)
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
        };
        // A feature: its header opens it, or folds it when already there.
        let group = |feature: Feature,
                     label: &'static str,
                     icon: Icon,
                     items: Vec<NavItem>,
                     cx: &mut Context<Self>| {
            NavGroup::new(
                SharedString::from(format!("nav-group-{label}")),
                label,
                icon,
            )
            .current(self.page.feature() == Some(feature))
            .expanded(!self.collapsed.contains(&feature))
            .items(items)
            .on_click(cx.listener(move |this, _, _, cx| this.open_or_fold(feature, cx)))
        };

        // The app's home, above the features; it opens on the tab last shown.
        let overview = NavItem::new(
            "nav-overview",
            "Overview",
            Icon::new(IconName::LayoutDashboard),
        )
        .active(matches!(self.page, Page::Overview(_)))
        .on_click(cx.listener(|this, _, _, cx| this.open_overview(cx)));
        // The wish counter: the banner types with wishes.
        let wish_pages = self
            .pools(cx)
            .into_iter()
            .map(|pool| {
                nested(
                    pool.as_str(),
                    pool.display_name().into(),
                    Page::Wishes(WishPage::History(pool)),
                    cx,
                )
            })
            .collect();
        let wishes = group(
            Feature::Wishes,
            "Wishes",
            Icon::new(gpui_kit::assets::IconName::Gem),
            wish_pages,
            cx,
        );
        // The game's characters and weapons.
        let archive_pages = vec![
            nested(
                "characters",
                "Characters".into(),
                Page::Archive(ArchivePage::Characters),
                cx,
            ),
            nested(
                "weapons",
                "Weapons".into(),
                Page::Archive(ArchivePage::Weapons),
                cx,
            ),
        ];
        let archive = group(
            Feature::Archive,
            "Archive",
            Icon::new(IconName::BookOpen),
            archive_pages,
            cx,
        );
        let planner = group(
            Feature::Planner,
            "Planner",
            Icon::new(gpui_kit::assets::IconName::Calculator),
            vec![nested("ascension", "Ascension".into(), Page::Planner, cx)],
            cx,
        );

        let brand = div().h_flex().gap_2().px_1().child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().sidebar_primary_foreground)
                .child(APP_TITLE),
        );

        let theme = cx.theme();
        div()
            .id("sidebar")
            .v_flex()
            .w(gpui_kit::rems(super::SIDEBAR_REMS))
            .h_full()
            .flex_shrink_0()
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                div()
                    .v_flex()
                    .gap_4()
                    .px_3()
                    .pt_4()
                    .pb_2()
                    .child(brand)
                    .when(has_accounts, |this| {
                        this.child(
                            picker("account", &self.account_select)
                                .accessibility_label("Account")
                                .menu_width(gpui_kit::rems(14.5)),
                        )
                    }),
            )
            .child(
                div()
                    .id("sidebar-pages")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .v_flex()
                            .gap_1()
                            .px_3()
                            .child(overview)
                            .child(wishes)
                            .child(archive)
                            .child(planner),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .px_3()
                    .py_3()
                    .child(item(
                        "settings",
                        "Settings".into(),
                        Icon::new(IconName::Settings),
                        Page::Settings,
                        cx,
                    ))
                    .child(item(
                        "credits",
                        "Credits".into(),
                        Icon::new(IconName::Info),
                        Page::Credits,
                        cx,
                    )),
            )
    }
}

/// An entry of the account picker: its name (and UID), standing for its id.
pub(super) fn account_option(summary: &AccountSummary) -> PickerItem<AccountId> {
    let account = &summary.account;
    // An account named after its UID (the default for a game import) shows
    // it once.
    let title = match &account.uid {
        Some(uid) if *uid != account.label => format!("{} · {uid}", account.label),
        _ => account.label.clone(),
    };
    PickerItem::new(title, account.id)
}

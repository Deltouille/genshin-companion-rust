//! Moving between pages, and keeping the account picker and the page shown
//! in step with the data.

use gpui_kit::{App, Context, Window};
use wish_core::model::{AccountId, Pool};
use wish_ui::PickerItem;

use super::sidebar::account_option;
use super::{ArchivePage, Feature, OverviewTab, Page, Shell, WishPage};

impl Shell {
    /// Lists the accounts in the picker, selects the session's account, and
    /// leaves a history page the account has no wishes for.
    pub(super) fn sync_account_picker(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let options: Vec<PickerItem<AccountId>> = self
            .library
            .read(cx)
            .accounts()
            .iter()
            .map(account_option)
            .collect();
        let account = self.session.read(cx).account();
        self.account_select.update(cx, |select, cx| {
            select.set_items(options, cx);
            if let Some(id) = account {
                select.set_selected_value(&id, cx);
            }
        });
        if let Page::Wishes(WishPage::History(pool)) = self.page
            && !self.pools(cx).contains(&pool)
        {
            self.page = self.first_wish_page(cx);
        }
        cx.notify();
    }

    /// The account whose wishes are shown.
    pub fn account(&self, cx: &App) -> Option<AccountId> {
        self.session.read(cx).account()
    }

    /// Opens `page`; for a banner type the selected account has no wishes
    /// in, its first one with wishes, or the overview.
    pub fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = match page {
            Page::Wishes(WishPage::History(pool)) if !self.pools(cx).contains(&pool) => {
                self.first_wish_page(cx)
            }
            page => page,
        };
        if let Page::Overview(tab) = self.page {
            self.overview_tab = tab;
        }
        // The group of the page shown stays open.
        if let Some(feature) = self.page.feature() {
            self.collapsed.remove(&feature);
        }
        if let Page::Wishes(WishPage::History(pool)) = self.page {
            self.history
                .update(cx, |history, cx| history.set_pool(pool, cx));
        }
        // The sidebar entry opens the list, even from a character's page.
        if self.page == Page::Archive(ArchivePage::Weapons) {
            self.weapons.update(cx, |weapons, cx| weapons.show_list(cx));
        }
        if self.page == Page::Archive(ArchivePage::Characters) {
            self.archive.update(cx, |archive, cx| archive.show_list(cx));
        }
        if let Page::Chronicle(page) = self.page {
            self.chronicle
                .update(cx, |chronicle, cx| chronicle.set_page(page, cx));
        }
        cx.notify();
    }

    /// Opens the overview on the tab it showed last (the sidebar's entry).
    pub fn open_overview(&mut self, cx: &mut Context<Self>) {
        self.navigate(Page::Overview(self.overview_tab), cx);
    }

    /// A feature group's header was clicked: away from the feature, it opens
    /// the feature; on one of its pages, it folds or unfolds the group.
    pub fn open_or_fold(&mut self, feature: Feature, cx: &mut Context<Self>) {
        if self.page.feature() == Some(feature) {
            if !self.collapsed.remove(&feature) {
                self.collapsed.insert(feature);
            }
            cx.notify();
        } else {
            self.navigate(feature.home(), cx);
        }
    }

    /// Opens the page of the character `key` (a genshin-db key).
    pub fn open_character(&mut self, key: &str, cx: &mut Context<Self>) {
        self.navigate(Page::Archive(ArchivePage::Characters), cx);
        self.archive
            .update(cx, |archive, cx| archive.show_character(key, cx));
    }

    /// Opens weapon `key`'s page in the archive.
    pub fn open_weapon(&mut self, key: &str, cx: &mut Context<Self>) {
        self.navigate(Page::Archive(ArchivePage::Weapons), cx);
        self.weapons
            .update(cx, |weapons, cx| weapons.show_weapon(key, cx));
    }

    /// On a history page, shows the banner view (`true`) or the wish list.
    pub fn show_banners(&mut self, banners: bool, cx: &mut Context<Self>) {
        self.history
            .update(cx, |history, cx| history.show_banners(banners, cx));
    }

    pub(super) fn pools(&self, cx: &App) -> Vec<Pool> {
        self.session.read(cx).pools(cx)
    }

    /// The wish counter's first banner type with wishes, or the overview's
    /// Wishes tab when there is none.
    fn first_wish_page(&self, cx: &App) -> Page {
        self.pools(cx)
            .first()
            .map_or(Page::Overview(OverviewTab::Wishes), |pool| {
                Page::Wishes(WishPage::History(*pool))
            })
    }
}

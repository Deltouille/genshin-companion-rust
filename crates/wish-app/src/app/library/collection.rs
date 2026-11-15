//! What an account owns: characters and weapon copies.

use super::*;

impl Library {
    /// Adds `character` to what `account` owns, or saves what changed.
    pub fn save_owned_character(
        &mut self,
        account: Option<AccountId>,
        character: OwnedCharacter,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let character = character.clamped();
        // Shown at once; the reload after the write confirms it.
        if let Some(holdings) = account.and_then(|a| self.holdings.get_mut(&a)) {
            let characters = &mut holdings.owned.characters;
            match characters.iter_mut().find(|c| c.key == character.key) {
                Some(stored) => *stored = character.clone(),
                None => {
                    characters.push(character.clone());
                    characters.sort_by(|a, b| a.key.cmp(&b.key));
                }
            }
            cx.notify();
        }
        self.write(
            cx,
            true,
            move |store, _| {
                let account = account_for_writing(store, account)?;
                Ok(store.save_owned_character(account, &character)?)
            },
            |_, ()| (),
        )
    }

    /// Forgets that `account` owns character `key` (its weapon stays).
    pub fn remove_owned_character(
        &mut self,
        account: AccountId,
        key: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.remove_owned_character(account, &key)?),
            |_, ()| (),
        )
    }

    /// Adds a copy of a weapon to `account`.
    pub fn add_owned_weapon(
        &mut self,
        account: Option<AccountId>,
        weapon: OwnedWeapon,
        cx: &mut Context<Self>,
    ) -> Task<Result<i64>> {
        let weapon = weapon.clamped();
        self.write(
            cx,
            true,
            move |store, _| {
                let account = account_for_writing(store, account)?;
                Ok(store.add_owned_weapon(account, &weapon)?)
            },
            |_, id| id,
        )
    }

    /// Saves what changed on a copy of a weapon.
    pub fn update_owned_weapon(
        &mut self,
        account: AccountId,
        weapon: OwnedWeapon,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let weapon = weapon.clamped();
        // Shown at once; the reload after the write confirms it.
        if let Some(holdings) = self.holdings.get_mut(&account) {
            let weapons = &mut holdings.owned.weapons;
            if weapon.holder.is_some() {
                for other in weapons.iter_mut().filter(|w| w.holder == weapon.holder) {
                    other.holder = None;
                }
            }
            if let Some(stored) = weapons.iter_mut().find(|w| w.id == weapon.id) {
                *stored = weapon.clone();
            }
            cx.notify();
        }
        self.write(
            cx,
            true,
            move |store, _| Ok(store.update_owned_weapon(account, &weapon)?),
            |_, ()| (),
        )
    }

    /// Forgets the copy `id` of a weapon.
    pub fn remove_owned_weapon(
        &mut self,
        account: AccountId,
        id: i64,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.remove_owned_weapon(account, id)?),
            |_, ()| (),
        )
    }
}

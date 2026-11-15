//! The plan and the inventory: each change shown at once, then written.

use super::*;

impl Library {
    /// Adds an entry at the end of `account`'s plan.
    pub fn add_plan_entry(
        &mut self,
        account: Option<AccountId>,
        target: PlanTarget,
        cx: &mut Context<Self>,
    ) -> Task<Result<i64>> {
        self.write(
            cx,
            true,
            move |store, _| {
                let account = account_for_writing(store, account)?;
                Ok(store.add_plan_entry(account, &target)?)
            },
            |_, id| id,
        )
    }

    /// Saves an entry's new goal, or a custom entry's new name and costs.
    pub fn update_plan_entry(
        &mut self,
        account: Option<AccountId>,
        entry: PlanEntry,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        // Shown at once; the reload after the write confirms it.
        if let Some(holdings) = account.and_then(|a| self.holdings.get_mut(&a))
            && let Some(stored) = holdings.plan.iter_mut().find(|e| e.id == entry.id)
        {
            *stored = entry.clone();
        }
        self.write(
            cx,
            true,
            move |store, _| Ok(store.update_plan_entry(&entry)?),
            |_, ()| (),
        )
    }

    /// Moves entry `id` of `account`'s plan to where entry `onto` is, the
    /// entries between shifting along: the order the inventory is shared out
    /// in.
    pub fn move_plan_entry(
        &mut self,
        account: Option<AccountId>,
        id: i64,
        onto: i64,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let Some((account, holdings)) =
            account.and_then(|a| Some(a).zip(self.holdings.get_mut(&a)))
        else {
            return Task::ready(Ok(()));
        };
        let plan = &mut holdings.plan;
        let from = plan.iter().position(|entry| entry.id == id);
        let to = plan.iter().position(|entry| entry.id == onto);
        let (Some(from), Some(to)) = (from, to) else {
            return Task::ready(Ok(()));
        };
        if from == to {
            return Task::ready(Ok(()));
        }
        // Shown at once; the reload after the write confirms it.
        let entry = plan.remove(from);
        plan.insert(to, entry);
        let ids: Vec<i64> = plan.iter().map(|entry| entry.id).collect();
        cx.notify();
        self.write(
            cx,
            true,
            move |store, _| Ok(store.reorder_plan(account, &ids)?),
            |_, ()| (),
        )
    }

    pub fn remove_plan_entry(&mut self, id: i64, cx: &mut Context<Self>) -> Task<Result<()>> {
        self.write(
            cx,
            true,
            move |store, _| Ok(store.remove_plan_entry(id)?),
            |_, ()| (),
        )
    }

    /// Records how many of `item` `account` has.
    pub fn set_owned(
        &mut self,
        account: Option<AccountId>,
        item: CustomItem,
        count: u64,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.set_owned_many(account, vec![(item, count)], cx)
    }

    /// Records several counts at once (the tiers of a series), in one write.
    pub fn set_owned_many(
        &mut self,
        account: Option<AccountId>,
        counts: Vec<(CustomItem, u64)>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        // Shown at once; the reload after the write confirms it.
        if let Some(holdings) = account.and_then(|a| self.holdings.get_mut(&a)) {
            for (item, count) in &counts {
                holdings.inventory.set(item, *count);
            }
            cx.notify();
        }
        self.write(
            cx,
            true,
            move |store, _| {
                let account = account_for_writing(store, account)?;
                Ok(store.set_owned_many(account, &counts)?)
            },
            |_, ()| (),
        )
    }
}

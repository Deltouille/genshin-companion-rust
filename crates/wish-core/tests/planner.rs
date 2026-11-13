//! The planner's costs against totals the game is known for.

use wish_core::planner::{CharacterGoal, Level, MORA, PlannerData, Requirement, WeaponGoal};

fn data() -> &'static PlannerData {
    PlannerData::embedded()
}

fn count(need: &Requirement, material: u32) -> u64 {
    need.materials.get(&material).copied().unwrap_or(0)
}

const FULL_TALENTS: [(u8, u8); 3] = [(1, 10); 3];

#[test]
fn the_embedded_data_is_complete() {
    let data = data();
    assert!(data.characters().len() > 120);
    assert!(data.weapons().len() > 250);
    // Every element of the Traveler is a character of its own.
    for element in ["anemo", "geo", "electro", "dendro", "hydro", "pyro"] {
        let traveler = data.character(&format!("traveler{element}")).unwrap();
        assert!(traveler.name.starts_with("Traveler ("));
    }
    assert!(data.material(MORA).is_some());
}

#[test]
fn levels_follow_the_ascension_caps() {
    assert!(Level::new(80, 5).is_some());
    assert!(Level::new(80, 6).is_some());
    assert!(Level::new(81, 5).is_none());
    assert!(Level::new(20, 0).is_some());
    assert!(Level::new(19, 1).is_none());
    assert!(Level::new(90, 7).is_none());
    assert_eq!(Level::new(80, 6).unwrap().cap(), 90);
}

#[test]
fn a_character_to_90_10_10_10() {
    let data = data();
    let ganyu = data.character("ganyu").unwrap();
    let need = data.character_cost(
        ganyu,
        &CharacterGoal {
            from: Level::START,
            to: Level::MAX,
            talents: FULL_TALENTS,
        },
    );

    // EXP from 1 to 90, and what it costs to give.
    assert_eq!(need.character_exp, 8_362_650);
    // Shivada Jade: 1 sliver, 9 fragments, 9 chunks, 6 gemstones.
    let jade = [104161, 104162, 104163, 104164].map(|id| count(&need, id));
    assert_eq!(jade, [1, 9, 9, 6]);
    assert_eq!(count(&need, 113010), 46, "Hoarfrost Core");
    assert_eq!(count(&need, 100031), 168, "Qingxin");
    // Nectars, from the ascension and three talents.
    let nectar = [112038, 112039, 112040].map(|id| count(&need, id));
    assert_eq!(nectar, [36, 96, 129]);
    // Diligence books for three talents to 10.
    let books = [104313, 104314, 104315].map(|id| count(&need, id));
    assert_eq!(books, [9, 63, 114]);
    assert_eq!(count(&need, 113015), 18, "Shadow of the Warrior");
    assert_eq!(count(&need, 104319), 3, "Crown of Insight");
    // Mora: 420,000 to ascend, 1,652,500 per talent.
    assert_eq!(count(&need, 202), 420_000 + 3 * 1_652_500);

    // The EXP as Hero's Wit alone, rounded up (8,362,650 EXP), and its Mora.
    let all = need.materials_with_exp();
    assert_eq!(all[&104003], 419, "Hero's Wit");
    assert!(!all.contains_key(&104002), "no Adventurer's Experience");
    assert!(!all.contains_key(&104001), "no Wanderer's Advice");
    assert_eq!(all[&202], 420_000 + 3 * 1_652_500 + 1_672_530);
}

#[test]
fn a_plan_adds_up_from_its_steps() {
    let data = data();
    let ganyu = data.character("ganyu").unwrap();
    let goal = |from, to, talents| CharacterGoal { from, to, talents };
    let mid = Level::new(50, 2).unwrap();
    let whole = data.character_cost(ganyu, &goal(Level::START, Level::MAX, FULL_TALENTS));
    let mut halves = data.character_cost(ganyu, &goal(Level::START, mid, [(1, 6); 3]));
    halves.merge(&data.character_cost(ganyu, &goal(mid, Level::MAX, [(6, 10); 3])));
    assert_eq!(whole, halves);
    // Nothing is counted for a step back.
    let back = data.character_cost(ganyu, &goal(Level::MAX, Level::START, [(10, 1); 3]));
    assert_eq!(back, Requirement::default());
}

#[test]
fn a_five_star_weapon_to_90() {
    let data = data();
    let amos = data.weapon("amosbow").unwrap();
    let need = data.weapon_cost(
        amos,
        &WeaponGoal {
            from: Level::START,
            to: Level::MAX,
        },
    );
    assert_eq!(need.weapon_exp, 9_064_450);
    // Dandelion Gladiator domain materials: 5, 14, 14, 6.
    let domain = [114009, 114010, 114011, 114012].map(|id| count(&need, id));
    assert_eq!(domain, [5, 14, 14, 6]);
    assert_eq!(count(&need, 202), 225_000);
    assert_eq!(need.materials_with_exp()[&202], 225_000 + 906_445);
}

#[test]
fn a_two_star_weapon_stops_at_70() {
    let data = data();
    let weapon = data
        .weapons()
        .iter()
        .find(|w| w.rarity == 2)
        .expect("a 2★ weapon");
    let need = data.weapon_cost(
        weapon,
        &WeaponGoal {
            from: Level::START,
            to: Level::MAX,
        },
    );
    let to_70 = data.weapon_cost(
        weapon,
        &WeaponGoal {
            from: Level::START,
            to: Level::new(70, 4).unwrap(),
        },
    );
    assert_eq!(need, to_70);
    assert!(need.weapon_exp > 0);
}

#[test]
fn a_file_with_an_unknown_material_is_refused() {
    let json = include_str!("../data/planner.json");
    // Ganyu's first ascension costs Shivada Jade Sliver (104161).
    let broken = json.replacen("[104161,1]", "[999999,1]", 1);
    assert_ne!(broken, json);
    let error = PlannerData::from_json(&broken).unwrap_err().to_string();
    assert!(error.contains("unknown material 999999"), "{error}");
}

#[test]
fn a_missing_file_falls_back_to_the_embedded_data() {
    let dir = tempfile::tempdir().unwrap();
    let data = PlannerData::load(dir.path()).unwrap();
    assert_eq!(data.source, PlannerData::embedded().source);
}

mod archive {
    use wish_core::archive::ArchiveData;
    use wish_core::planner::{Level, PlannerData};

    #[test]
    fn every_planned_character_has_a_page() {
        let archive = ArchiveData::embedded();
        for character in PlannerData::embedded().characters() {
            let page = archive.character(&character.key).expect("a page");
            assert_eq!(page.constellations.len(), 6, "{}", character.key);
            assert!(page.talents.len() >= 5, "{}", character.key);
        }
    }

    #[test]
    fn stats_at_a_level_match_the_game() {
        let archive = ArchiveData::embedded();
        let ganyu = archive.character("ganyu").unwrap();
        let at_90 = archive.stats(ganyu, Level::MAX);
        assert_eq!(at_90.hp.round(), 9797.0);
        assert_eq!(at_90.attack.round(), 335.0);
        assert_eq!(at_90.defense.round(), 630.0);
        assert!(
            (at_90.special - 0.884).abs() < 1e-9,
            "CRIT DMG {}",
            at_90.special
        );
        let at_1 = archive.stats(ganyu, Level::START);
        assert_eq!(
            (at_1.hp.round(), at_1.attack.round(), at_1.defense.round()),
            (763.0, 26.0, 49.0)
        );
        // Ascending at 80 raises the stats without a level.
        let before = archive.stats(ganyu, Level::new(80, 5).unwrap());
        let after = archive.stats(ganyu, Level::new(80, 6).unwrap());
        assert!(after.hp > before.hp);
    }

    #[test]
    fn talent_values_at_a_level() {
        let archive = ArchiveData::embedded();
        let ganyu = archive.character("ganyu").unwrap();
        let normal = &ganyu.talents[0];
        assert_eq!(normal.kind, "Normal Attack");
        let at = |level| normal.values(level);
        assert_eq!(at(1)[0], ("1-Hit DMG".to_owned(), "31.7%".to_owned()));
        assert_eq!(at(10)[0].1, "62.7%");
        assert_eq!(
            at(10)[8],
            ("Frostflake Arrow DMG".to_owned(), "230%".to_owned())
        );
        let skill = &ganyu.talents[1];
        assert_eq!(skill.values(1)[2], ("Duration".to_owned(), "6s".to_owned()));
        // Passives have no levels.
        assert!(!ganyu.talents[3].has_levels());
    }
}

mod plan {
    use wish_core::AppPaths;
    use wish_core::Store;
    use wish_core::planner::{
        CharacterGoal, CustomCost, CustomItem, Level, PlanEntry, PlanTarget, PlannerData,
    };

    fn ganyu(talents: [(u8, u8); 3]) -> PlanTarget {
        PlanTarget::Character {
            key: "ganyu".to_owned(),
            goal: CharacterGoal {
                from: Level::START,
                to: Level::MAX,
                talents,
            },
        }
    }

    #[test]
    fn entries_are_kept_edited_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let account = store.add_account("Main", None, None).unwrap().id;
        assert!(store.plan(account).unwrap().is_empty());

        let ganyu_id = store.add_plan_entry(account, &ganyu([(1, 10); 3])).unwrap();
        let leak = PlanTarget::Custom {
            name: "Leaked character".to_owned(),
            costs: vec![
                CustomCost {
                    item: CustomItem::Material(202),
                    count: 420_000,
                },
                CustomCost {
                    item: CustomItem::Named("New boss drop".to_owned()),
                    count: 46,
                },
            ],
        };
        let leak_id = store.add_plan_entry(account, &leak).unwrap();
        let plan = store.plan(account).unwrap();
        assert_eq!(
            plan,
            vec![
                PlanEntry {
                    id: ganyu_id,
                    target: ganyu([(1, 10); 3])
                },
                PlanEntry {
                    id: leak_id,
                    target: leak.clone()
                },
            ]
        );

        store
            .update_plan_entry(&PlanEntry {
                id: ganyu_id,
                target: ganyu([(6, 9); 3]),
            })
            .unwrap();
        // The order is the player's: it decides who gets the inventory first.
        store.reorder_plan(account, &[leak_id, ganyu_id]).unwrap();
        let order: Vec<i64> = store.plan(account).unwrap().iter().map(|e| e.id).collect();
        assert_eq!(order, [leak_id, ganyu_id]);

        store.remove_plan_entry(leak_id).unwrap();
        let plan = store.plan(account).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].target, ganyu([(6, 9); 3]));
    }

    #[test]
    fn a_custom_entry_needs_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let account = store.add_account("Main", None, None).unwrap().id;
        let blank = PlanTarget::Custom {
            name: "  ".to_owned(),
            costs: Vec::new(),
        };
        assert!(store.add_plan_entry(account, &blank).is_err());
        assert!(store.plan(account).unwrap().is_empty());
    }

    #[test]
    fn totals_add_up_characters_and_custom_entries() {
        let data = PlannerData::embedded();
        let plan = [
            PlanEntry {
                id: 1,
                target: ganyu([(1, 10); 3]),
            },
            PlanEntry {
                id: 2,
                target: PlanTarget::Custom {
                    name: "Leak".to_owned(),
                    costs: vec![
                        CustomCost {
                            item: CustomItem::Material(113010),
                            count: 4,
                        },
                        CustomCost {
                            item: CustomItem::Named("New gem".to_owned()),
                            count: 6,
                        },
                    ],
                },
            },
            PlanEntry {
                id: 3,
                target: PlanTarget::Character {
                    key: "nobody".to_owned(),
                    goal: CharacterGoal {
                        from: Level::START,
                        to: Level::MAX,
                        talents: [(1, 1); 3],
                    },
                },
            },
        ];
        let totals = data.plan_totals(&plan);
        // Ganyu's 46 Hoarfrost Cores and the leak's 4.
        assert_eq!(totals.materials[&113010], 50);
        assert_eq!(totals.named["New gem"], 6);
        assert_eq!(totals.unknown, vec![3]);
        assert_eq!(data.material_named(" hero's wit "), Some(104003));
    }
}

mod custom_characters {
    use wish_core::AppPaths;
    use wish_core::Store;
    use wish_core::planner::{
        CharacterGoal, CustomItem, CustomMaterials, Inventory, Level, PlanEntry, PlanTarget,
        PlannerData, Role,
    };

    fn data() -> &'static PlannerData {
        PlannerData::embedded()
    }

    /// Ganyu's materials, as a custom character would pick them.
    fn ganyus_materials() -> CustomMaterials {
        let data = data();
        let roles = data.roles(data.character("ganyu").unwrap()).unwrap();
        roles
            .into_iter()
            .map(|(role, tiers)| (role, CustomItem::Material(tiers[0])))
            .collect()
    }

    #[test]
    fn every_character_but_the_traveler_has_six_roles() {
        let data = data();
        for character in data.characters() {
            let roles = data.roles(character);
            assert_eq!(
                roles.is_none(),
                character.key.starts_with("traveler"),
                "{}",
                character.key
            );
        }
        let ganyu = data.roles(data.character("ganyu").unwrap()).unwrap();
        let names = |role: Role| -> Vec<&str> {
            ganyu[&role]
                .iter()
                .map(|id| data.material(*id).unwrap().name.as_str())
                .collect()
        };
        assert_eq!(names(Role::LocalSpecialty), ["Qingxin"]);
        assert_eq!(names(Role::BossMaterial), ["Hoarfrost Core"]);
        assert_eq!(
            names(Role::Common),
            ["Whopperflower Nectar", "Shimmering Nectar", "Energy Nectar"]
        );
        assert_eq!(
            names(Role::TalentBook),
            [
                "Teachings of Diligence",
                "Guide to Diligence",
                "Philosophies of Diligence"
            ]
        );
        assert_eq!(names(Role::ElementalGem).len(), 4);
    }

    #[test]
    fn each_role_offers_the_series_characters_use() {
        let data = data();
        // The seven elements; the Traveler's Brilliant Diamond is not offered.
        assert_eq!(data.families(Role::ElementalGem).len(), 7);
        for role in Role::ALL {
            let families = data.families(*role);
            assert!(families.len() > 5, "{role}");
            assert!(families.iter().all(|tiers| tiers.len() == role.tiers()));
            let first = families[0][0];
            assert_eq!(data.family(*role, first), Some(families[0].as_slice()));
        }
    }

    #[test]
    fn a_custom_character_costs_what_the_character_it_copies_costs() {
        let data = data();
        let ganyu = data.character("ganyu").unwrap();
        let goals = [
            CharacterGoal::FULL,
            CharacterGoal {
                from: Level::new(40, 1).unwrap(),
                to: Level::new(80, 6).unwrap(),
                talents: [(1, 6), (6, 9), (8, 10)],
            },
        ];
        for goal in goals {
            let (need, named) = data.custom_character_cost(&ganyus_materials(), &goal);
            assert!(named.is_empty());
            assert_eq!(need, data.character_cost(ganyu, &goal));
        }
    }

    #[test]
    fn materials_the_data_lacks_are_counted_by_name() {
        let data = data();
        let mut materials = ganyus_materials();
        materials.insert(
            Role::LocalSpecialty,
            CustomItem::Named("Moon Lily".to_owned()),
        );
        materials.insert(Role::TalentBook, CustomItem::Named("Ardour".to_owned()));
        materials.remove(&Role::WeeklyMaterial);
        let (need, named) = data.custom_character_cost(&materials, &CharacterGoal::FULL);
        assert_eq!(named["Moon Lily"], 168);
        assert_eq!(named["Ardour (2★)"], 9);
        assert_eq!(named["Ardour (3★)"], 63);
        assert_eq!(named["Ardour (4★)"], 114);
        // A role left out is named after the role.
        assert_eq!(named["Weekly material"], 18);
        assert_eq!(need.materials[&202], 420_000 + 3 * 1_652_500);
    }

    #[test]
    fn custom_characters_are_kept_and_edited() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let account = store.add_account("Main", None, None).unwrap().id;
        let mut materials = ganyus_materials();
        materials.insert(Role::BossMaterial, CustomItem::Named("New boss".to_owned()));
        let leak = PlanTarget::CustomCharacter {
            name: "Leaked character".to_owned(),
            stars: 5,
            materials,
            goal: CharacterGoal::FULL,
        };
        let id = store.add_plan_entry(account, &leak).unwrap();
        assert_eq!(
            store.plan(account).unwrap(),
            [PlanEntry {
                id,
                target: leak.clone()
            }]
        );

        let PlanTarget::CustomCharacter {
            mut materials,
            mut goal,
            ..
        } = leak
        else {
            unreachable!()
        };
        materials.remove(&Role::BossMaterial);
        goal.talents = [(6, 9); 3];
        let edited = PlanTarget::CustomCharacter {
            name: "Renamed".to_owned(),
            stars: 4,
            materials,
            goal,
        };
        store
            .update_plan_entry(&PlanEntry {
                id,
                target: edited.clone(),
            })
            .unwrap();
        assert_eq!(store.plan(account).unwrap()[0].target, edited);

        store.remove_plan_entry(id).unwrap();
        assert!(store.plan(account).unwrap().is_empty());
    }

    #[test]
    fn the_inventory_keeps_what_the_player_enters() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let account = store.add_account("Main", None, None).unwrap().id;
        assert_eq!(store.inventory(account).unwrap(), Inventory::default());

        let mora = CustomItem::Material(202);
        let leak = CustomItem::Named("Moon Lily".to_owned());
        store.set_owned(account, &mora, 360_000).unwrap();
        store.set_owned(account, &leak, 12).unwrap();
        store.set_owned(account, &mora, 420_000).unwrap();
        let inventory = store.inventory(account).unwrap();
        assert_eq!(inventory.owned(&mora), 420_000);
        assert_eq!(inventory.owned(&leak), 12);
        assert_eq!(inventory.owned(&CustomItem::Material(104003)), 0);

        // None of it forgets it.
        store.set_owned(account, &leak, 0).unwrap();
        let inventory = store.inventory(account).unwrap();
        assert_eq!(inventory.owned(&leak), 0);
        assert!(inventory.named.is_empty());
    }
}

mod allocation {
    use wish_core::planner::{
        CharacterGoal, CustomItem, Inventory, Level, MORA, PlanEntry, PlanTarget, PlannerData,
    };

    const HEROS_WIT: u32 = 104003;
    const ADVENTURERS_EXPERIENCE: u32 = 104002;
    const WANDERERS_ADVICE: u32 = 104001;

    fn entry(id: i64, key: &str, goal: CharacterGoal) -> PlanEntry {
        PlanEntry {
            id,
            target: PlanTarget::Character {
                key: key.to_owned(),
                goal,
            },
        }
    }

    fn owning(items: &[(u32, u64)]) -> Inventory {
        let mut inventory = Inventory::default();
        for &(id, count) in items {
            inventory.set(&CustomItem::Material(id), count);
        }
        inventory
    }

    #[test]
    fn the_inventory_goes_to_the_first_entries_first() {
        let data = PlannerData::embedded();
        let plan = [
            entry(1, "albedo", CharacterGoal::FULL),
            entry(2, "columbina", CharacterGoal::FULL),
        ];
        let nothing = data.allocate(&plan, &Inventory::default());
        let mora = |share: usize| nothing.entries[share].needed.materials[&MORA];
        let (first, second) = (mora(0), mora(1));

        // Not enough for the first: it lacks the rest, the second all of it.
        let some = data.allocate(&plan, &owning(&[(MORA, 5_000_000)]));
        assert_eq!(some.entries[0].missing_material(MORA), first - 5_000_000);
        assert_eq!(some.entries[1].missing_material(MORA), second);

        // Enough for the first: the second gets what is left.
        let more = first + 1_000_000;
        let plenty = data.allocate(&plan, &owning(&[(MORA, more)]));
        assert_eq!(plenty.entries[0].missing_material(MORA), 0);
        assert_eq!(plenty.entries[1].missing_material(MORA), second - 1_000_000);
        assert_eq!(
            plenty.missing.materials[&MORA],
            first + second - more,
            "the plan lacks what its entries lack"
        );
    }

    #[test]
    fn a_tier_short_is_crafted_from_the_tiers_below() {
        let data = PlannerData::embedded();
        let ganyu = data.character("ganyu").unwrap();
        let roles = data.roles(ganyu).unwrap();
        let books = &roles[&wish_core::planner::Role::TalentBook];
        let (teachings, guide, philosophies) = (books[0], books[1], books[2]);
        // One talent from 1 to 10: 3 teachings, 21 guides, 38 philosophies.
        let plan = [entry(
            1,
            "ganyu",
            CharacterGoal {
                from: Level::MAX,
                to: Level::MAX,
                talents: [(1, 10), (1, 1), (1, 1)],
            },
        )];
        let inventory = owning(&[(teachings, 30), (guide, 21), (philosophies, 34)]);
        let share = &data.allocate(&plan, &inventory).entries[0];
        // The teachings and guides it needs are its own; the 27 teachings
        // left make 9 guides, then 3 philosophies: one still lacks.
        assert_eq!(share.missing_material(teachings), 0);
        assert_eq!(share.missing_material(guide), 0);
        assert_eq!(share.missing_material(philosophies), 1);
    }

    #[test]
    fn exp_books_count_for_their_exp() {
        let data = PlannerData::embedded();
        let plan = [
            entry(1, "ganyu", CharacterGoal::FULL),
            entry(2, "albedo", CharacterGoal::FULL),
        ];
        let needed = data.allocate(&plan, &Inventory::default());
        let wits = needed.entries[0].needed.materials[&HEROS_WIT];
        // Adventurer's Experience instead of Hero's Wit: 4 for 1, enough for
        // the first character and some to spare for the second.
        let inventory = owning(&[(ADVENTURERS_EXPERIENCE, 4 * (wits + 10))]);
        let shared = data.allocate(&plan, &inventory);
        for book in [HEROS_WIT, ADVENTURERS_EXPERIENCE, WANDERERS_ADVICE] {
            assert_eq!(shared.entries[0].missing_material(book), 0);
        }
        assert!(shared.entries[1].missing_material(HEROS_WIT) < wits - 5);
    }

    #[test]
    fn what_is_still_needed_of_exp_shows_as_heros_wit_alone() {
        let data = PlannerData::embedded();
        let plan = [entry(1, "ganyu", CharacterGoal::FULL)];
        // 8,362,650 EXP to level 90; 3 Wanderer's Advice and 2 Adventurer's
        // Experience give 13,000 of it.
        let inventory = owning(&[(WANDERERS_ADVICE, 3), (ADVENTURERS_EXPERIENCE, 2)]);
        let shared = data.allocate(&plan, &inventory);
        assert_eq!(
            shared.missing.materials.get(&HEROS_WIT),
            Some(&(8_362_650u64 - 13_000).div_ceil(20_000))
        );
        for book in [ADVENTURERS_EXPERIENCE, WANDERERS_ADVICE] {
            assert!(!shared.needed.materials.contains_key(&book));
            assert!(!shared.missing.materials.contains_key(&book));
        }
    }

    #[test]
    fn the_spare_of_a_lower_tier_is_crafted_up() {
        let data = PlannerData::embedded();
        // Ganyu to 90 with her talents to 10 needs 36, 96 and 129 nectar.
        let nectar = [112038, 112039, 112040];
        let plan = [entry(1, "ganyu", CharacterGoal::FULL)];
        let needed = data.allocate(&plan, &Inventory::default());
        assert_eq!(nectar.map(|id| needed.needed.materials[&id]), [36, 96, 129]);
        // 151 of the first tier: 36 for the plan, 114 of the 115 left make
        // 38 of the second, which then lacks 96 - 45 - 38 = 13. Nothing is
        // left to craft the third.
        let inventory = owning(&[(nectar[0], 151), (nectar[1], 45), (nectar[2], 21)]);
        let shared = data.allocate(&plan, &inventory);
        let missing = nectar.map(|id| shared.missing.materials.get(&id).copied().unwrap_or(0));
        assert_eq!(missing, [0, 13, 108]);
        assert_eq!(nectar.map(|id| shared.crafted(id)), [0, 38, 0]);
        assert_eq!(nectar.map(|id| shared.entries[0].crafted(id)), [0, 38, 0]);
    }
}

mod weapon_pages {
    use wish_core::archive::ArchiveData;
    use wish_core::planner::{Level, PlannerData};

    #[test]
    fn every_planned_weapon_has_a_page() {
        let archive = ArchiveData::embedded();
        for weapon in PlannerData::embedded().weapons() {
            let page = archive.weapon(&weapon.key).expect("a page");
            assert!(page.art.is_some(), "{}", weapon.key);
            // 1★ and 2★ weapons have neither a secondary stat nor a passive.
            assert_eq!(
                page.stats.special.is_some(),
                weapon.rarity > 2,
                "{}",
                weapon.key
            );
            // Five refinements, or one for the few that can't be refined.
            if let Some(passive) = &page.passive {
                let count = passive.refinements.len();
                assert!(count == 5 || count == 1, "{}: {count}", weapon.key);
            }
        }
    }

    #[test]
    fn weapon_stats_at_a_level_match_the_game() {
        let archive = ArchiveData::embedded();
        let amos = archive.weapon("amosbow").unwrap();
        let at_90 = archive.weapon_stats(amos, Level::MAX);
        assert_eq!(at_90.attack.round(), 608.0);
        let atk_share = at_90.special.unwrap();
        assert!((atk_share - 0.496).abs() < 0.001, "ATK% {atk_share}");
        let at_1 = archive.weapon_stats(amos, Level::START);
        assert_eq!(at_1.attack.round(), 46.0);
        // Elemental Mastery is a number.
        let sacrificial = archive.weapon("sacrificialfragments").unwrap();
        assert!(!sacrificial.stats.special_percent);
        let em = archive
            .weapon_stats(sacrificial, Level::MAX)
            .special
            .unwrap();
        assert_eq!(em.round(), 221.0);
    }
}

mod weapons_in_the_plan {
    use wish_core::AppPaths;
    use wish_core::Store;
    use wish_core::planner::{
        CustomItem, Inventory, Level, PlanEntry, PlanTarget, PlannerData, WeaponGoal,
    };

    const MYSTIC_ORE: u32 = 104013;

    #[test]
    fn a_weapon_tops_out_by_its_rarity() {
        assert_eq!(*Level::weapon_steps(5).last().unwrap(), Level::MAX);
        assert_eq!(
            *Level::weapon_steps(2).last().unwrap(),
            Level::new(70, 4).unwrap()
        );
        assert_eq!(WeaponGoal::full(1).to, Level::new(70, 4).unwrap());
    }

    #[test]
    fn weapon_entries_are_kept_and_costed() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let account = store.add_account("Main", None, None).unwrap().id;
        let amos = PlanTarget::Weapon {
            key: "amosbow".to_owned(),
            goal: WeaponGoal::full(5),
        };
        let id = store.add_plan_entry(account, &amos).unwrap();
        assert_eq!(
            store.plan(account).unwrap(),
            [PlanEntry {
                id,
                target: amos.clone()
            }]
        );
        let lower = PlanTarget::Weapon {
            key: "amosbow".to_owned(),
            goal: WeaponGoal {
                from: Level::new(40, 1).unwrap(),
                to: Level::new(80, 6).unwrap(),
            },
        };
        store
            .update_plan_entry(&PlanEntry {
                id,
                target: lower.clone(),
            })
            .unwrap();
        assert_eq!(store.plan(account).unwrap()[0].target, lower);

        // Its EXP is ores, and owned ores count for their EXP.
        let data = PlannerData::embedded();
        let plan = [PlanEntry { id, target: amos }];
        let needed = data.allocate(&plan, &Inventory::default());
        let ores = needed.entries[0].needed.materials[&MYSTIC_ORE];
        assert!(ores > 400, "{ores} Mystic Enhancement Ores to 90");
        let mut inventory = Inventory::default();
        inventory.set(&CustomItem::Material(MYSTIC_ORE), 100);
        let shared = data.allocate(&plan, &inventory);
        assert_eq!(shared.entries[0].missing_material(MYSTIC_ORE), ores - 100);
    }
}

mod custom_weapons {
    use wish_core::AppPaths;
    use wish_core::Store;
    use wish_core::planner::{
        CustomItem, CustomMaterials, Level, PlanEntry, PlanTarget, PlannerData, Role, WeaponGoal,
    };

    /// A weapon's own materials, as a custom weapon would pick them.
    fn materials_of(key: &str) -> CustomMaterials {
        let data = PlannerData::embedded();
        let roles = data.weapon_roles(data.weapon(key).unwrap()).unwrap();
        roles
            .into_iter()
            .map(|(role, tiers)| (role, CustomItem::Material(tiers[0])))
            .collect()
    }

    #[test]
    fn a_custom_weapon_costs_what_a_weapon_of_its_stars_costs() {
        let data = PlannerData::embedded();
        for (key, stars) in [("amosbow", 5), ("thewidsith", 4)] {
            let weapon = data.weapon(key).unwrap();
            for goal in [
                WeaponGoal::full(stars),
                WeaponGoal {
                    from: Level::new(50, 2).unwrap(),
                    to: Level::new(80, 6).unwrap(),
                },
            ] {
                let (need, named) = data.custom_weapon_cost(&materials_of(key), stars, &goal);
                assert!(named.is_empty());
                assert_eq!(need, data.weapon_cost(weapon, &goal), "{key}");
            }
        }
        // Weapons have three roles, each with the series of the data.
        let roles = data.weapon_roles(data.weapon("amosbow").unwrap()).unwrap();
        assert_eq!(roles.keys().copied().collect::<Vec<_>>(), {
            let mut weapon = Role::WEAPON.to_vec();
            weapon.sort();
            weapon
        });
        assert!(data.families(Role::WeaponMaterial).len() >= 20);
        assert!(data.families(Role::EliteDrop).len() >= 15);
    }

    #[test]
    fn custom_weapons_are_kept_and_edited() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(AppPaths::at(dir.path())).unwrap();
        let account = store.add_account("Main", None, None).unwrap().id;
        let mut materials = materials_of("amosbow");
        materials.insert(Role::EliteDrop, CustomItem::Named("New drop".to_owned()));
        let leak = PlanTarget::CustomWeapon {
            name: "Leaked bow".to_owned(),
            stars: 5,
            materials,
            goal: WeaponGoal::full(5),
        };
        let id = store.add_plan_entry(account, &leak).unwrap();
        assert_eq!(
            store.plan(account).unwrap(),
            [PlanEntry {
                id,
                target: leak.clone()
            }]
        );
        let edited = PlanTarget::CustomWeapon {
            name: "Leaked bow".to_owned(),
            stars: 4,
            materials: materials_of("thewidsith"),
            goal: WeaponGoal {
                from: Level::new(20, 1).unwrap(),
                to: Level::MAX,
            },
        };
        store
            .update_plan_entry(&PlanEntry {
                id,
                target: edited.clone(),
            })
            .unwrap();
        assert_eq!(store.plan(account).unwrap()[0].target, edited);
    }
}

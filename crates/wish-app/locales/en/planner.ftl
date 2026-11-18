# The ascension planner: the plan's cards, what it still needs, adding a
# character, a weapon or a custom one, the inventory and the resin estimate.

planner-title = Ascension planner
planner-subtitle = What raising your characters and weapons costs
planner-add-character = Add a character…
planner-add-weapon = Add a weapon…
planner-custom = Custom…
planner-custom-tooltip = For a character or a weapon the game data does not have yet: a name and its materials
planner-empty = Nothing planned yet
planner-empty-help = Add a character or a weapon to see what raising it costs, or a custom one the game data does not have yet.
planner-not-saved = The plan was not saved
planner-remove-title = Remove { $name } from the plan?
planner-remove-help = Its goal is forgotten. A backup is taken first.
planner-remove = Remove
planner-remove-tooltip = Remove { $name } from the plan
planner-edit = Edit { $name }
planner-drag = Drag to change the order: the first characters take from your inventory first

## A card

planner-level = Level
planner-level-of = { $name }’s level
planner-talent-of = { $name }’s { $talent }
planner-normal-attack = Normal Attack
planner-elemental-skill = Elemental Skill
planner-elemental-burst = Elemental Burst
planner-character-gone = This character is no longer in the game data.
planner-weapon-gone = This weapon is no longer in the game data.
planner-custom-character = Custom character · { $stars }★
planner-custom-weapon = Custom weapon · { $stars }★
planner-custom-entry = Custom entry
planner-nothing-to-spend = Nothing to spend.

## A material's tooltip

planner-tip-covered = { $name }: { $need } needed, covered by your inventory
planner-tip-partly = { $name }: { $need } needed, { $lack } still to get
planner-tip-needed = { $name }: { $need } needed
planner-tip-plan = { $name }: { $need } for the whole plan, { $owned } owned, { $lack } still to get
planner-tip-crafted = { $tip }; { $crafted } crafted from the tiers below
planner-tip-unknown = { $tip } (not in the game data)

## Still needed

planner-today = Farm today
planner-today-day = { $day }, { $server } server
planner-today-day-local = { $day }, this computer’s clock
planner-today-help = The domains open today that give what the plan still lacks. They change every day at 4:00, server time; on Sunday, every domain is open.
planner-today-help-local = The domains open today that give what the plan still lacks. They change every day at 4:00, server time, but the account’s server isn’t known: the day follows this computer’s clock. Give the account its UID or server in Settings → Accounts.
planner-today-none = No domain gives what the plan lacks today: the next day one does is { $day }.
planner-today-for = For
planner-still-needed = Still needed
planner-entries = { $count ->
        [one] { $count } entry
       *[other] { $count } entries
    }
planner-still-needed-help = What the plan lacks once your inventory is shared out, in the plan’s order; lower tiers are crafted up. Click a material to enter how many you have.

## Adding a character or a weapon

planner-search-character = Search a character
planner-search-weapon = Search a weapon
planner-owned-level = Lv. { $level } · C{ $constellation }
planner-planned = Planned
planner-owned = Owned
planner-not-owned = Not owned
planner-every-weapon = Every weapon
planner-section-count = { $title } · { $count }
planner-no-match = Nothing matches this search.
planner-add-character-title = Add a character
planner-add-weapon-title = Add a weapon

## A custom character or weapon

planner-name = Name
planner-search = Search…
planner-type-name = Not listed? Type its name
planner-type-name-first = Type the material’s name first.
planner-name-character = Give the character a name.
planner-name-weapon = Give the weapon a name.
# Something the form still lacks, by role.
planner-choose-its = { $role ->
        [common] Choose its common drop.
        [local-specialty] Choose its local specialty.
        [boss-material] Choose its boss material.
        [elemental-gem] Choose its elemental gem.
        [talent-book] Choose its talent book.
        [weekly-material] Choose its weekly material.
        [weapon-material] Choose its weapon material.
       *[elite-drop] Choose its elite drop.
    }
# The title of a role's choice.
planner-choose-the = { $role ->
        [common] Choose the common drop
        [local-specialty] Choose the local specialty
        [boss-material] Choose the boss material
        [elemental-gem] Choose the elemental gem
        [talent-book] Choose the talent book
        [weekly-material] Choose the weekly material
        [weapon-material] Choose the weapon material
       *[elite-drop] Choose the elite drop
    }
planner-character = Character
planner-weapon = Weapon
planner-choose = Choose…
planner-stars = Stars
planner-back = Back
planner-nothing-in-data = Nothing in the game data matches.
planner-use-name = Use this name
planner-edit-weapon-title = Edit the custom weapon
planner-edit-character-title = Edit the custom character
planner-add-custom-title = Add a custom character or weapon
planner-save = Save
planner-add = Add

## The inventory

planner-inventory = Inventory
planner-change-hint = +60,000 or -20
planner-fewer-than-none = You can’t have fewer than none.
planner-whole-numbers = Type whole numbers, such as 360,000 or -20.
planner-nothing-still-needed = nothing still needed
planner-n-still-needed = { $count } still needed
planner-plan-needs-line = The plan needs { $count }, { $still }
planner-you-have = You have
planner-add-or-subtract = Add or subtract
planner-ley-line = Add a ley line (+{ $mora })
planner-exp = { $exp } EXP
# EXP books: "4 Adventurer’s Experience or 20 Wanderer’s Advice do a Hero’s Wit’s work."
planner-exp-note = Each counts for its EXP: { $others } do a { $top }’s work.
planner-or = { $first } or { $second }
planner-craft-note = What a tier lacks is crafted from the spare of the tiers below, three for one: what is still needed is what is left to farm.
planner-plan-needs = The plan needs
planner-crafted = Crafted

## The resin estimate

planner-days = { NUMBER($days, minimumFractionDigits: 1, maximumFractionDigits: 1) } { $days ->
        [one] day
       *[other] days
    }
planner-runs = { $count ->
        [one] { $count } run
       *[other] { $count } runs
    }
planner-resin-source = { $source }: { $runs }, { $resin } resin
planner-resin = ~{ $resin } resin
planner-resin-days = · ~{ $days }
planner-resin-count = { $resin } resin
planner-resin-options = Resin options…
planner-resin-nothing = Nothing still lacking is farmed with resin.
planner-resin-no-source = No source is counted: choose them in the options.
planner-resin-about = About { $days } at { $per_day } resin a day, world level { $level }
planner-resin-per-day-error = Resin a day is a whole number from 1 to { $most }.
planner-world-level = World level
planner-world-level-help = Bosses give more materials as it rises; ley lines give their most from 6.
planner-resin-per-day = Resin a day
planner-resin-per-day-help = 180 regenerate in a day; add what you refill or take from Fragile Resin.
planner-resin-sources = Count
planner-resin-sources-help = Domains count at their highest level. Weekly bosses, gems, ores, enemy drops and local specialties take no resin counted here.
planner-resin-title = Resin estimate
planner-resin-not-saved = The resin options were not saved

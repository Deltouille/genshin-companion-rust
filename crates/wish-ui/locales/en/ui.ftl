# The design system's texts: numbers, dates and the components' own words.

## Numbers. `value` is already written with the language's digits.

ui-percent = { $value }%
ui-thousands = { $value }K
ui-millions = { $value }M
ui-kilobytes = { $value } KB
ui-megabytes = { $value } MB
ui-gigabytes = { $value } GB

## Dates: `day` and `year` are numbers, `month` a month's name below.

ui-date = { $day } { $month } { $year }
ui-date-time = { $date }, { $time }
ui-date-at-time = { $date } at { $time }
ui-month-year = { $month } { $year }

ui-month-short-1 = Jan
ui-month-short-2 = Feb
ui-month-short-3 = Mar
ui-month-short-4 = Apr
ui-month-short-5 = May
ui-month-short-6 = Jun
ui-month-short-7 = Jul
ui-month-short-8 = Aug
ui-month-short-9 = Sep
ui-month-short-10 = Oct
ui-month-short-11 = Nov
ui-month-short-12 = Dec

ui-month-1 = January
ui-month-2 = February
ui-month-3 = March
ui-month-4 = April
ui-month-5 = May
ui-month-6 = June
ui-month-7 = July
ui-month-8 = August
ui-month-9 = September
ui-month-10 = October
ui-month-11 = November
ui-month-12 = December

## The 50/50

ui-guaranteed-rate-up = guaranteed rate-up
ui-outcome-won = Won
ui-outcome-lost = Lost
ui-outcome-guaranteed = Guaranteed
# The banner data doesn't match the wish: never happens in the game.
ui-outcome-unexpected = Unexpected

## Quotes, around a name the player gave or a file's name

ui-quoted = “{ $text }”

## Controls

ui-cancel = Cancel
ui-close = Close
ui-choose = Choose…
ui-search = Search
ui-nothing-matches = Nothing matches.
# A picker read aloud: its name, then its choice.
ui-picker-label = { $label }: { $choice }
ui-previous = Previous
ui-next = Next
ui-page = Page { $page }
ui-pages = Pages { $first } to { $last }
# A sidebar group read aloud.
ui-expanded = { $label }, expanded
ui-collapsed = { $label }, collapsed
ui-current-page = { $label }, current page

## The 5★ pity chart

ui-pity-title = 5★ pity
ui-pity-average = average { $average }
ui-pity-one = One 5★ so far, at pity { $pity }.
ui-pity-none = No 5★ yet.
ui-pity = Pity

## The game's elements and weapon types, as the game names them

ui-element-pyro = Pyro
ui-element-hydro = Hydro
ui-element-anemo = Anemo
ui-element-electro = Electro
ui-element-dendro = Dendro
ui-element-cryo = Cryo
ui-element-geo = Geo
ui-weapon-sword = Sword
ui-weapon-claymore = Claymore
ui-weapon-polearm = Polearm
ui-weapon-bow = Bow
ui-weapon-catalyst = Catalyst

## Stats, as the game names them

ui-stat-hp = HP
ui-stat-atk = ATK
ui-stat-def = DEF
ui-stat-base-hp = Base HP
ui-stat-base-atk = Base ATK
ui-stat-base-def = Base DEF
ui-stat-crit-rate = CRIT Rate
ui-stat-crit-dmg = CRIT DMG
ui-stat-energy-recharge = Energy Recharge
ui-stat-elemental-mastery = Elemental Mastery
ui-stat-healing-bonus = Healing Bonus
ui-stat-physical-dmg = Physical DMG Bonus
ui-stat-pyro-dmg = Pyro DMG Bonus
ui-stat-hydro-dmg = Hydro DMG Bonus
ui-stat-anemo-dmg = Anemo DMG Bonus
ui-stat-electro-dmg = Electro DMG Bonus
ui-stat-dendro-dmg = Dendro DMG Bonus
ui-stat-cryo-dmg = Cryo DMG Bonus
ui-stat-geo-dmg = Geo DMG Bonus

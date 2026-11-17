# The overview, the app's home: the account in figures, a tab each for its
# wishes, characters and weapons.

overview-title = Overview
overview-uid = UID { $uid }
overview-no-wishes = No wishes yet
overview-no-wishes-help = Import your wish history from the game, or your paimon.moe data: in paimon.moe, open Settings and choose Export, or download the save file from your Google Drive. Import both to keep the history paimon.moe holds from before the game’s last six months.
overview-no-wishes-manual = Or open a banner in the sidebar to count your wishes by hand.
# Under a figure: "of 97", what it is counted against.
overview-of = of { $total }

## Characters tab

overview-no-characters = No characters yet
overview-no-characters-help = Sync them from HoYoLAB, or mark the characters you have in the archive: they are counted here, by rarity, element, weapon and constellation.
overview-open-characters = Open the characters
overview-characters = Characters
overview-level = Level { $level }
overview-crowns = { $count ->
        [one] { $count } talent
       *[other] { $count } talents
    } at level { $level }
overview-by-element = By element
overview-by-weapon = By weapon
overview-constellations = Constellations

## Weapons tab

overview-no-weapons = No weapons yet
overview-no-weapons-help = Mark the weapons you have in the archive, or sync them from HoYoLAB with the characters holding them: they are counted here, by rarity and type.
overview-open-weapons = Open the weapons
overview-weapons = Weapons
overview-refined-fully = Refined to R5
overview-copies-held = { $held } of { $copies ->
        [one] { $copies } copy
       *[other] { $copies } copies
    } held
overview-by-type = By type
overview-five-star-weapons = 5★ weapons
overview-four-star-weapons = 4★ weapons
overview-lesser-weapons = And { $count ->
        [one] { $count } weapon
       *[other] { $count } weapons
    } of 3★ or less, in the archive.
overview-copies = { $count } copies

## Wishes tab

overview-wishes = Wishes
overview-primogems = { $count } primogems
overview-average-pity = Average pity { $pity }
overview-none-yet = None yet
overview-rate-won = 50/50 won
overview-won-lost = { $won } won · { $lost } lost
overview-guaranteed = Guaranteed
overview-guaranteed-at = Guaranteed at { $pity }
overview-five-pity = 5★ pity
overview-four-pity = 4★ pity
overview-history = { $pool } history
overview-wish-count = { $count ->
        [one] { $count } wish
       *[other] { $count } wishes
    }
overview-outside-banners = { $count ->
        [one] { $count } wish falls
       *[other] { $count } wishes fall
    } outside every banner this version knows: update the app.
overview-unknown-items = { $count ->
        [one] { $count } wish has
       *[other] { $count } wishes have
    } an item this version doesn’t know.
overview-manual-counters = Counters typed by hand in paimon.moe.
overview-total = Total
overview-share = Share
overview-average = Avg. pity
overview-character = Character
overview-weapon = Weapon
# A row of the 5★/4★ table: "Won 50/50" or "Won 75/25".
overview-won-odds = Won { $odds }
overview-no-five = No 5★ yet.
overview-every-five = Every 5★ and its pity, latest first
overview-pill-won = won the { $odds }
overview-pill-lost = lost the { $odds }
overview-pill-guaranteed = guaranteed
overview-per-month = Wishes per month
overview-one-month = All { $count ->
        [one] { $count } wish so far was
       *[other] { $count } wishes so far were
    } made in { $month }: { $four } × 4★, { $five } × 5★.
overview-month-wishes = { $count } ({ $primogems } primogems)

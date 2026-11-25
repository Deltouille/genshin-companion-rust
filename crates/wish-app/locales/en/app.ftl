# The app's own texts: the menus, the shell, what every page shares.

app-menu-import-game = Import from Game…
app-menu-import-file = Import File…
app-menu-sync-hoyolab = Sync from HoYoLAB…
app-menu-import-good = Import a GOOD file…
app-menu-quit = Quit

app-close = Close
app-cancel = Cancel

# What didn't happen, then why: "Couldn’t export: the disk is full".
app-failure = { $what }: { $why }
app-folder-picker-failed = Couldn’t open the folder picker

## The sidebar, and the overview's tabs (the same words)

app-nav-overview = Overview
app-nav-wishes = Wishes
app-nav-characters = Characters
app-nav-weapons = Weapons
app-nav-resources = Resources
app-nav-archive = Archive
app-nav-planner = Planner
app-nav-ascension = Ascension
app-nav-chronicle = Chronicle
app-nav-abyss = Spiral Abyss
app-nav-theater = Imaginarium Theater
app-nav-diary = Traveler’s Diary
app-nav-import = Import
app-nav-settings = Settings
app-nav-credits = Credits
app-account = Account

## Opening the data

app-opening = Opening your wish history…
app-open-failed = Couldn’t open your wish history
app-open-failed-help = Your data is untouched. It is stored in { $folder }, with automatic backups in its backups folder.
app-show-data-folder = Show data folder
app-opening-database = opening the wish database
app-not-open = your data isn’t open yet

## Before the window: the app can't start

app-cant-start = { $app } can’t start
app-error-file = Also written to { $file }.
app-folder-failed = The data folder could not be prepared: { $error }.
app-already-open = { $app } is already open. Only one window at a time can use your data folder.

## The HoYoLAB dialog's title

app-sync-hoyolab-title = Sync from HoYoLAB
app-import-good-title = Import a GOOD file

## Writes, exports and imports

app-write-panicked = an internal error stopped it: { $detail }
app-write-panic = a panic
app-write-cut-off = the change was cut off before it was saved
app-write-window-closed = the window closed before the change was saved
app-import-too-large = { $file } is { $size }, too large for a wish history export
app-choose-account-first = choose an account first
app-exported = Exported { $wishes ->
        [one] { $wishes } wish
       *[other] { $wishes } wishes
    } of { $accounts ->
        [one] { $accounts } account
       *[other] { $accounts } accounts
    }.
app-exported-left-out = Exported { $wishes ->
        [one] { $wishes } wish
       *[other] { $wishes } wishes
    } of { $accounts ->
        [one] { $accounts } account
       *[other] { $accounts } accounts
    }. Left out, without a UID: { $left_out }.
app-exported-account = Exported the wishes of { $account }.
app-images-busy = images are downloading: stop that first

## The chronicle synced on its own, with the HoYoLAB login the keyring keeps

app-chronicle-kept = Chronicle synced from HoYoLAB: { $added ->
        [0] { $updated ->
            [one] a period brought up to date
           *[other] { $updated } periods brought up to date
        }
        [one] a period kept
       *[other] { $added } periods kept
    }.
app-chronicle-up-to-date = The chronicle already has everything HoYoLAB shows.
app-chronicle-paused = HoYoLAB asks for a captcha: the chronicle won’t sync on its own until the app opens again
app-chronicle-failed = The chronicle didn’t sync
app-chronicle-syncing = The chronicle is syncing already.
app-login-kept = The cookie is kept in this system’s keyring: the chronicle now syncs on its own.
app-no-login-kept = No HoYoLAB cookie is kept on this computer.
app-login-refused = HoYoLAB no longer accepts the cookie kept, so it was forgotten. Sync from HoYoLAB again and tick “Remember” to keep a new one.
app-forget-login-failed = The HoYoLAB cookie wasn’t forgotten
app-settings-save-failed = Couldn’t save the setting

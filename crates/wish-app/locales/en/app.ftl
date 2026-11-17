# The app's own texts: the menus, the shell, what every page shares.

app-menu-import-game = Import from Game…
app-menu-import-file = Import File…
app-menu-sync-hoyolab = Sync from HoYoLAB…
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
app-nav-archive = Archive
app-nav-planner = Planner
app-nav-ascension = Ascension
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

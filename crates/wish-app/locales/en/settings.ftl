# Settings: your data, its backups and mirror, recent imports, export,
# counting by hand, the game data, the images and the language.

settings-title = Settings
settings-save-failed = Couldn’t save the setting

## Language

settings-group-language = Language
settings-language = Interface language
settings-language-help = The app’s texts, and the game’s names and descriptions.
# The system's language, named in itself: "System (Français)".
settings-language-system = System ({ $language })

## Your data

settings-group-data = Your data
settings-data-folder = Data folder
settings-data-folder-help = Your wish database, its backups and a copy of every imported file.
settings-reveal = Show in file manager
settings-backups = Backups
settings-backups-summary = { $count ->
        [one] { $count } backup
       *[other] { $count } backups
    }, the latest on { $latest }. Taken before every import, undo or restore and once a day (the { $kept } most recent are kept), and once an hour while counting by hand or editing the plan (the { $kept_hourly } most recent).
settings-backups-none = No backups yet. One is taken before every import or undo, and once a day.
settings-back-up-now = Back up now
settings-restore = Restore…
settings-mirror-folder = Mirror folder
settings-mirror-help = A copy of all your wishes, characters, weapons, plan and inventory, updated a few seconds after each change and when the app closes. Choose a folder synced by Syncthing, Nextcloud, Dropbox or similar to keep a copy off this computer.
settings-mirror-written = Last written at { $time }.
settings-mirror-failed = Couldn’t write the last copy: { $error }
settings-mirror-pending = Written after the next change.
settings-mirror-stop = Stop mirroring
settings-mirror-choose = Choose folder…
settings-mirror-prompt = Mirror here
settings-mirror-change-failed = Couldn’t change the mirror folder

## Backups

settings-backup-failed = Couldn’t back up
settings-backup-saved = Backup saved.
settings-restore-title = Restore a backup
settings-restore-dialog-help = Restoring replaces everything with what the backup holds: your accounts and wishes, your characters and weapons, and the planner’s plan and inventory. The current state is backed up first.
settings-restore-choose-file = Choose file…
settings-restore-prompt = Restore
# What is restored, in a question: "Restore the backup from 9 Nov 2026, 12:20:05?"
settings-restore-what-backup = the backup from { $when }
settings-restore-confirm-title = Restore { $what }?
settings-restore-confirm-help = Everything is replaced by what the backup holds: your accounts and wishes, your characters and weapons, and the planner’s plan and inventory. The current state is backed up first, so you can restore it if needed.
settings-restore-button = Restore
settings-restore-failed = Couldn’t restore
settings-restore-done = Backup restored. The previous state was backed up.

settings-backup-reason-daily = Daily backup
settings-backup-reason-import = Before an import
settings-backup-reason-migration = Before an app update
settings-backup-reason-manual = Backed up by hand
settings-backup-reason-undo = Before undoing an import
settings-backup-reason-restore = Before restoring a backup
settings-backup-reason-delete = Before deleting wishes
settings-backup-reason-counting = Before counting wishes
settings-backup-reason-planning = Before changing your collection or plan
settings-backup-reason-sync = Before a HoYoLAB sync
settings-backup-reason-account = Before removing an account
settings-backup-reason-unknown = Backup

## Recent imports

settings-group-imports = Recent imports
settings-imports-none = No imports yet
settings-import-undone = undone
settings-import-added = { $count } added
settings-undo = Undo…
settings-origin-game = The game
settings-origin-manual = Counted by hand
settings-origin-manual-what = Counted by hand: { $what }
settings-origin-file = A file
settings-undo-game-title = Undo the game import of { $time }?
settings-undo-manual-title = Undo { $what }, counted by hand?
settings-undo-manual-wishes = the wishes
settings-undo-file-title = Undo importing { $origin }?
settings-undo-help = This removes the { $count ->
        [one] wish
       *[other] { $count } wishes
    } it added to { $account }. A backup is taken first.
settings-undo-button = Undo import
settings-undo-failed = Couldn’t undo the import
settings-undo-done = { $count ->
        [one] Removed 1 wish.
       *[other] Removed { $count } wishes.
    }

## Export

settings-group-export = Export
settings-export = Export…
settings-export-failed = Couldn’t export
settings-export-snapshot = Snapshot
settings-export-snapshot-help = Every account, with nothing left out: wishes, characters, weapons, plan and inventory. Import it back here, on this or another computer.
settings-export-uigf = UIGF
settings-export-uigf-help = For other trackers such as Snap Hutao or Starward. Accounts need a UID.
settings-export-spreadsheet = Spreadsheet
settings-export-spreadsheet-help = The account on screen, in paimon.moe’s Excel layout.

## Counting

settings-group-counting = Counting
settings-manual-counting = Count wishes by hand
settings-manual-counting-help = Adds +1, +10 and “Add 4★ or 5★…” to each banner page, like paimon.moe’s manual mode. Each entry shows in Recent imports and can be undone there.

## Game data

settings-group-game-data = Game data
settings-banners-items = Banners and items
settings-banners-items-help = Used to work out banners and rate-up results.
settings-banners-items-source = Used to work out banners and rate-up results. From paimon.moe, commit { $commit }.
settings-game-data-folder = Your game-data folder
settings-game-data-partial = Not all of it is used: { $warning }.

## Images

settings-group-images = Images
settings-images-downloaded = Downloaded images
settings-images-help = { $count ->
        [one] { $count } image
       *[other] { $count } images
    }, { $size }. Pages download the images they show, once, from paimon.moe and Project Amber; they stay on this computer.
settings-images-empty = Empty…
settings-images-offline = For offline use
settings-images-offline-help = Download them now, so that every page has its pictures without a connection. Icons cover the wish counter and the planner; everything adds the banner art and the characters’ wish art.
settings-images-stopping = Stopping…
settings-images-downloading = Downloading { $done } of { $total }…
settings-images-progress = Images downloaded
# `missing`: images neither site has.
settings-images-done = { $missing ->
        [0] Every image is downloaded.
       *[other] Every image is downloaded ({ $missing } not on paimon.moe or Project Amber).
    }
settings-images-failed = { $missing ->
        [0] { $failed } couldn’t be downloaded: try again later.
       *[other] { $failed } couldn’t be downloaded ({ $missing } not on paimon.moe or Project Amber): try again later.
    }
settings-images-stopped = Stopped: { $stored } of { $total } downloaded.
settings-images-unreachable = Couldn’t reach paimon.moe or Project Amber. Check the connection, then try again.
settings-images-icons = Icons
settings-images-everything = Everything
# A download button once nothing is left: "Icons: done".
settings-images-offer-done = { $what }: done
# A download button: "Icons (~25 MB)".
settings-images-offer-size = { $what } (~{ $size })
settings-images-stop = Stop
settings-images-empty-title = Empty the downloaded images?
settings-images-empty-help = They are downloaded again when pages show them. None of your data is touched.
settings-images-empty-button = Empty
settings-images-empty-failed = Couldn’t empty the downloaded images

## Accounts
settings-group-accounts = Accounts
settings-account-uid = UID { $uid }
settings-account-no-uid = No UID
settings-account-no-server = server unknown
settings-account-wishes = { $count ->
        [0] no wishes
        [one] { $count } wish
       *[other] { $count } wishes
    }
settings-account-characters = { $count ->
        [0] no characters
        [one] { $count } character
       *[other] { $count } characters
    }
settings-account-edit = Edit…
settings-account-remove = Remove…
settings-account-add-title = Add an account
settings-account-add-help = For a player who imports no wishes yet, or another account of yours. Imports and HoYoLAB make their accounts on their own.
settings-account-add = Add…
settings-account-add-button = Add
settings-account-added = { $account } added
settings-account-add-failed = Couldn’t add the account
settings-account-edit-title = Edit the account
settings-account-save = Save
settings-account-edit-failed = Couldn’t change the account
settings-account-remove-title = Remove { $account }?
settings-account-remove-help = Its characters, weapons, plan and inventory go with it. A backup is taken first: Restore… brings them back.
settings-account-remove-button = Remove
settings-account-remove-failed = Couldn’t remove the account
settings-account-removed = { $account } removed
settings-account-name = Name
settings-account-uid-label = UID (optional)
settings-account-uid-placeholder = 9 or 10 digits
settings-account-server = Server
settings-account-server-unknown = Not known
settings-account-uid-kept = It came with an import or was given once: the imports find the account by it, so it stays.
settings-account-identity-help = A UID says its server. Without one, choose the server: the wish banners’ times and the planner’s day follow it. Once given, the UID stays.

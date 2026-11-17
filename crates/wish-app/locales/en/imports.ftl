# The Import page, and the HoYoLAB sync's dialog.

imports-title = Import
imports-sync-button = Sync from HoYoLAB…
imports-wish-history = Wish history
imports-wish-history-game = From the game: the app finds the wish history link in the game’s cache (Windows, or a Wine or Proton prefix on Linux), or you paste it from any device, then asks HoYoverse’s server for your wishes.
imports-wish-history-file = From a file: a paimon.moe save or spreadsheet, a UIGF v4 file from another tracker, or this app’s snapshot.
imports-collection = Characters and weapons
imports-collection-help = From HoYoLAB: your characters as they are in the game (level, constellations, talents) and the weapon each holds, into your collection. Your HoYoLAB cookie is used for that sync only, and never saved.
imports-collection-safe = It only adds or raises what you entered, never lowers it, and shows what would change before writing anything.
imports-good = Characters, weapons and materials
imports-good-help = From a GOOD file, which Irminsul, Inventory Kamera and Genshin Optimizer export: your characters, every weapon in your bag and your materials, into the account shown. The planner then counts with your real materials.
imports-good-safe = Characters and weapons are only added or raised, never lowered; the materials take the file’s counts. A preview shows every change before anything is written.
imports-good-button = Import a GOOD file…
imports-undo = Every import can be undone, and a backup is taken before each: see Settings.

## The HoYoLAB dialog: the cookie

hoyolab-intro = HoYoLAB shows your characters as they are in the game: their level, constellations and talents, and the weapon each holds. The app reads them with your HoYoLAB cookie, for this sync only: the cookie is never saved, and goes to HoYoverse only. Nothing is written before you confirm.
hoyolab-warning = These values let whoever has them use your HoYoLAB account. Give them to this app only: never to a website, a Discord bot or another person.
hoyolab-step-1 = 1. Log in to hoyolab.com in your browser.
hoyolab-step-2 = 2. Open its developer tools (F12), then Storage (Application in Chrome or Edge) → Cookies → https://www.hoyolab.com.
hoyolab-step-3 = 3. Copy the values of ltuid_v2, ltoken_v2 and ltmid_v2 into the fields below.
hoyolab-pasted = Cookie read from the clipboard ({ $names }); the clipboard was emptied. If your system keeps a clipboard history (Win+V, Klipper…), delete the entry there too.
hoyolab-use-fields = Use the fields
hoyolab-paste = Paste a whole cookie
hoyolab-fine-print = Typing the three values is safer than pasting: a clipboard history keeps what is copied. HoYoLAB doesn’t show the weapons in your bag or your materials. Characters at a level cap (20, 40 … 80) count as not yet ascended, since HoYoLAB doesn’t say. HoYoLAB has no public API: this uses the one its website uses, which HoYoverse may change or block.
hoyolab-fetch = Fetch from HoYoLAB
hoyolab-fill-fields = Fill in the three fields, or paste the whole cookie.

## The HoYoLAB dialog: the preview

hoyolab-asking = Asking HoYoLAB for your characters…
hoyolab-account = { $nickname } · AR { $level }
hoyolab-choose-target = No account here has UID { $uid } yet. Choose the one whose wish history is this player’s: it takes the UID, so the next import from the game goes there too.
hoyolab-heading = UID { $uid } · { $nickname }, { $into }
hoyolab-nothing-new = Your collection already has all of this.
hoyolab-sync = Sync
hoyolab-try-again = Try again
hoyolab-synced = { $nickname }’s characters are synced from HoYoLAB: { $count ->
        [one] { $count } change
       *[other] { $count } changes
    }.
hoyolab-new-account = A new account, { $name }
hoyolab-into = into { $account }
hoyolab-into-new = into a new account, { $name }
hoyolab-to-add = { $count } to add
hoyolab-to-raise = { $count } to raise
hoyolab-to-move = { $count } to move
hoyolab-characters = Characters: { $parts }
hoyolab-weapons = Weapons: { $parts }
hoyolab-nothing = Nothing to change.
hoyolab-character-line = { $name } · { $level } · C{ $constellation } · talents { $talents }
hoyolab-was = { $after }  (was { $before })
hoyolab-weapon-line = { $weapon } · { $level } · R{ $refinement }
hoyolab-weapon-held = { $weapon } · { $level } · R{ $refinement } · held by { $holder }
hoyolab-moved = { $weapon } · { $level } · R{ $refinement }: from { $from } to { $to }
hoyolab-nobody = nobody

## The GOOD import's dialog

good-prompt = Choose a GOOD file
good-intro = A GOOD file holds your characters, the weapons in your bag and your materials. It goes into the account shown, { $account }: to import into another, pick it in the sidebar first.
good-intro-new = A GOOD file holds your characters, the weapons in your bag and your materials. It goes into a new account, “Main”.
good-how-irminsul = Irminsul: start it, then the game; once it has read your data, export it to a file or to the clipboard.
good-how-kamera = Inventory Kamera: scan your account, then choose the file it writes.
good-how-optimizer = Genshin Optimizer: export your database, then choose the file.
good-safe = Characters and weapons are only added or raised, never lowered. The materials the planner uses take the file’s counts, those the file lacks go to 0, and the Mora is kept. Nothing is written before you confirm, and a backup comes first.
good-paste = Paste from the clipboard
good-choose-file = Choose a file…
good-clipboard-empty = The clipboard holds no text: copy the GOOD export first, or choose the file.
good-reading = Reading the file…
good-heading = From { $source }, { $into }
good-heading-unnamed = From a GOOD file, { $into }
good-into = into { $account }
good-into-new = into a new account, “Main”
good-materials = Materials: { $count } to change
good-section-characters = Characters
good-section-weapons = Weapons
good-section-materials = Materials
good-grouped = { $line } ×{ $count }
good-material-line = { $name }: { $before } → { $after }
good-nothing-new = Your collection and your inventory already have all of this.
good-import = Import
good-imported = The GOOD file is imported into { $account }: { $count ->
        [one] { $count } change
       *[other] { $count } changes
    }.

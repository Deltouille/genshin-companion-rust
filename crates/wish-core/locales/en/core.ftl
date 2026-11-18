# The core's texts: errors and the notes it writes for the player.
# An error follows what failed and a colon ("Couldn’t import: …"), so it
# starts lowercase.

## Errors

error-database = database error: { $detail }
error-io-reading = couldn’t read { $path }: { $detail }
error-io-writing = couldn’t write { $path }: { $detail }
error-io-creating = couldn’t create { $path }: { $detail }
error-io-copying = couldn’t copy { $path }: { $detail }
error-io-copying-data-folder = couldn’t copy the data folder to { $path }: { $detail }
error-io-deleting = couldn’t delete { $path }: { $detail }
error-io-flushing = couldn’t save { $path } to the disk: { $detail }
error-io-listing = couldn’t list { $path }: { $detail }
error-io-locking = couldn’t lock { $path }: { $detail }
error-io-opening = couldn’t open { $path }: { $detail }
error-io-removing = couldn’t remove { $path }: { $detail }
error-io-renaming = couldn’t rename { $path }: { $detail }
error-io-replacing = couldn’t replace { $path }: { $detail }
error-json = invalid JSON: { $detail }
error-no-data-dir = could not find a data directory for this user
error-schema-too-new = a newer version of Genshin Companion wrote this database (schema { $found }; this one reads up to { $known }). It is left untouched: update the app to open it
error-game-data = invalid game data: { $detail }
error-not-paimon-export = this isn’t a paimon.moe save: it holds no wish history
error-unrecognized-file = this file is neither a paimon.moe save (Settings → Export, or the Google Drive file) nor a Genshin Companion snapshot
error-not-good-file = this isn’t a GOOD file: export one from Irminsul, Inventory Kamera or Genshin Optimizer
error-unknown-items = the file has items this version of Genshin Companion doesn’t know yet: { $items }. Update the app, then import the file again
error-invalid-pull = `{ $key }` pull #{ $index }: { $detail }
error-invalid-entry = `{ $key }`: { $detail }
error-not-snapshot = this isn’t a Genshin Companion snapshot (format “{ $found }”, not “{ $expected }”)
error-snapshot-too-new = a newer version of Genshin Companion wrote this snapshot (format { $found }; this one reads up to { $known }): update the app to import it
error-not-a-backup = { $file } isn’t a backup of this app’s database
error-damaged-backup = { $file } is damaged ({ $problem }); choose another backup
error-excel = couldn’t read or write the spreadsheet: { $detail }
error-not-paimon-excel = this spreadsheet isn’t a paimon.moe wish history export
error-unknown-account = there is no account { $account }
error-empty-account-name = an account needs a name
error-invalid-uid = “{ $uid }” isn’t a UID: a UID has 9 or 10 digits
error-uid-taken = UID { $uid } is already the account “{ $account }”
error-account-has-uid = “{ $account }” already has its UID, which the imports find it by: it can’t be changed
error-account-has-wishes = “{ $account }” has wishes or imports in its history, so it can’t be removed: the wish history is what the app keeps
error-account-mismatch = “{ $account }” already belongs to another UID, so the wishes of UID { $uid } can’t be added to it
error-not-manual = wish { $wish } wasn’t entered by hand, so it can’t be deleted; undo its import instead
error-unknown-import = there is no import { $import }
error-already-undone = import { $import } was already undone
error-invalid-link = this isn’t a wish history link. Copy the whole link: it starts with https:// and contains “authkey”
error-link-expired = the wish history link has expired. Open the wish history in the game again, then use the new link
error-link-rejected = the game server didn’t accept this link. Copy the whole link again; if it still fails, open the wish history in the game again for a new one
error-api = the game server refused the request: { $message } ({ $retcode })
error-network = couldn’t reach the game server: { $detail }
error-unexpected-response = unexpected answer from the game server: { $detail }
error-image-download = couldn’t download the image { $url }: { $problem }
error-mixed-accounts = the history mixes two accounts (UIDs { $first } and { $second }); use a link from one account
error-cancelled = cancelled

## HoYoLAB's errors: whole sentences, shown alone.

hoyolab-missing-cookie = The cookie lacks { $field }. Copy the whole Cookie header of a request to hoyolab.com.
hoyolab-invalid-cookie = HoYoLAB doesn’t accept this cookie. Log in to hoyolab.com again, then copy a new one.
hoyolab-data-not-public = HoYoLAB keeps this data private. On hoyolab.com, open your profile’s Battle Chronicle settings and make it public, then try again.
hoyolab-captcha = HoYoLAB asks for a captcha. Open your Battle Chronicle on hoyolab.com once, solve it there, then try again.
hoyolab-too-frequent = HoYoLAB says the requests come too often. Wait a minute, then try again.
hoyolab-no-game-account = No Genshin Impact account is linked to this HoYoLAB account.
hoyolab-not-hoyoverse = { $host } isn’t one of HoYoverse’s addresses: the cookie is sent to those only.
hoyolab-api = HoYoLAB answered [{ $retcode }] { $message }
hoyolab-network = Couldn’t reach HoYoLAB: { $detail }
hoyolab-format = HoYoLAB’s answer doesn’t read: { $detail }

## settings.json

settings-unreadable = settings.json could not be read ({ $error }); the defaults are used
settings-set-aside = settings.json could not be read ({ $error }); it was kept as { $aside } and the defaults are used
settings-not-set-aside = settings.json could not be read ({ $error }), nor set aside ({ $problem })

## Banner types

pool-beginners = Beginners’ Wish
pool-standard = Standard
pool-character-event = Character Event
pool-weapon-event = Weapon Event
pool-chronicled = Chronicled Wish

# The same, where they are listed under "Wishes" (the sidebar), so may be shorter.
pool-short-beginners = Beginners’ Wish
pool-short-standard = Standard
pool-short-character-event = Character Event
pool-short-weapon-event = Weapon Event
pool-short-chronicled = Chronicled Wish

## Imports

import-paimon-old-format = stored in paimon.moe’s old format; open the wish counter on paimon.moe once so it converts the data, then export again
import-paimon-out-of-order = { $account }, { $pool }: { $count ->
        [one] { $count } wish is
       *[other] { $count } wishes are
    } not in time order in paimon.moe’s data; they are sorted by time here, which can change their pity
import-paimon-foreign-codes = { $account }, { $pool }: { $count ->
        [one] { $count } wish
       *[other] { $count } wishes
    } carried another banner’s gacha type; recorded as { $code }
import-held-back = { $count ->
        [one] { $count } older wish from { $pool } was
       *[other] { $count } older wishes from { $pool } were
    } not added: this banner’s history has wishes typed by hand in paimon.moe, which they could repeat.
import-unknown-banner-type = { $count ->
        [one] { $count } wish
       *[other] { $count } wishes
    } from an unknown banner type ({ $code }) left out; update the app.
import-conflicts = { $account }, { $pool }: { $count ->
        [one] { $count } wish differs
       *[other] { $count } wishes differ
    } from the stored wish with the same id; the stored version was kept
import-counters-kept = { $account }, { $pool }: kept the stored manual counters (total { $stored_total }, 5★ pity { $stored_five }, 4★ pity { $stored_four }) instead of the imported ones (total { $total }, 5★ pity { $five }, 4★ pity { $four })

## HoYoLAB sync: what the player should know

sync-phase-unknown = { $name } is level { $level }: HoYoLAB doesn’t say whether they ascended, so they count as not ascended yet. Set it on their page if they did.
sync-above-max = { $name } is level { $level }: counted as 90/90, the highest this version knows.
sync-unknown-character = { $name } isn’t in this version’s game data: left out, with their weapon. Update the app to sync them.
sync-unknown-weapon = The weapon { $name } isn’t in this version’s game data: left out. Update the app to sync it.
sync-talents-unread = { $name }’s talents couldn’t be read: left as they were. Set them on their page.
sync-good-unknown-character = { $name } isn’t in this version’s game data: left out. Update the app to import them.
sync-good-unknown-weapon = The weapon { $name } isn’t in this version’s game data: { $copies ->
        [one] its copy is left out.
       *[other] its { $copies } copies are left out.
    } Update the app to import it.
sync-other-items = { $count ->
        [one] One entry of the file isn’t a material the planner uses (food, a gadget, a quest item, or a material newer than this version): left out.
       *[other] { $count } entries of the file aren’t materials the planner uses (food, gadgets, quest items, or materials newer than this version): left out.
    }
sync-no-mora = The file has no Mora: the inventory’s is left as it is.
sync-no-materials = The file has no materials: the inventory is left as it is.

## The planner: what each material does for a custom character or weapon

role-common = Common
role-local-specialty = Local specialty
role-boss-material = Boss material
role-elemental-gem = Elemental gem
role-talent-book = Talent book
role-weekly-material = Weekly material
role-weapon-material = Weapon material
role-elite-drop = Elite drop

## The resin estimate's sources, and where each is farmed

resin-talent-books = Talent books
resin-weapon-materials = Weapon materials
resin-boss-materials = Boss materials
resin-character-exp = Character EXP
resin-mora = Mora
resin-place-talent-books = Domains of Mastery
resin-place-weapon-materials = Domains of Forgery
resin-place-boss-materials = Normal bosses
resin-place-character-exp = Blossoms of Revelation
resin-place-mora = Blossoms of Wealth

## Talent values, as the game writes them

archive-percent = { $value }%

## The planner's placeholder materials, "?", for one not known yet

placeholder-common = Unknown common drop
placeholder-local-specialty = Unknown local specialty
placeholder-boss-material = Unknown boss material
placeholder-elemental-gem = Unknown gem
placeholder-talent-book = Unknown talent book
placeholder-weekly-material = Unknown weekly boss material
placeholder-weapon-material = Unknown weapon material
placeholder-elite-drop = Unknown elite drop
# A tier of one: "Unknown talent book (2★)".
placeholder-tier = { $name } ({ $stars }★)

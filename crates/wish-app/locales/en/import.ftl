# Importing wishes: a file (paimon.moe, a snapshot, UIGF, a spreadsheet), or
# the history behind the game's link.

import-file-button = Import file…
import-game-button = Import from game…
import-prompt = Import
import-file-picker-failed = Couldn’t open the file picker
import-read-failed = Couldn’t read { $name }
import-title = Import { $name }?
import-button = Import
import-game-title = Import from the game
import-failed = Couldn’t import { $name }
import-nothing-changed = Nothing was changed.
import-ok = OK

## The report

import-nothing-new-game = Nothing new: every wish from the game was already stored.
import-nothing-new-file = Nothing new: every wish in { $name } was already stored.
import-linked-only = No new wishes. { $count ->
        [one] { $count } stored wish
       *[other] { $count } stored wishes
    } matched the game’s records.
import-added = { $count ->
        [one] Added { $count } new wish.
       *[other] Added { $count } new wishes.
    }
import-with-extras = { $headline } { $count ->
        [one] { $count } change
       *[other] { $count } changes
    } to your characters, weapons, plan or inventory.

## The preview of a file

import-carried = And { $list }
import-characters = { $count ->
        [one] { $count } character
       *[other] { $count } characters
    }
import-weapons = { $count ->
        [one] { $count } weapon
       *[other] { $count } weapons
    }
import-plan-entries = { $count ->
        [one] { $count } plan entry
       *[other] { $count } plan entries
    }
import-materials = { $count ->
        [one] { $count } material
       *[other] { $count } materials
    }
import-check-match = Pity and 50/50 results match paimon.moe for all { $count } values it stored.
import-check-differ = { $differ } of { $checked } values differ from what paimon.moe stored. This happens when paimon.moe’s banner data changed after your last import there. Your wishes are imported unchanged either way.
import-preview-help = Wishes that are already stored stay unchanged, and a backup is taken first. Characters and weapons are added or raised, never lowered; plan entries and materials you already have are kept as they are.

## Import from the game

import-game-help = Open the wish history in the game (Wish, then History), then find its link here or paste it. A link works for about a day. On a phone or a console, copy the link there and paste it here.
import-find-link = Find link
import-choose-game-folder = Choose game folder…
import-choose-prompt = Choose
import-fetch = Fetch history
import-link-found = Found the link of your last visit to the wish history. If that was more than a day ago, open the wish history in the game again first.
import-link-not-found = No link found. Open the wish history in the game (Wish, then History), then try again, or choose the game folder.
import-folder-not-remembered = The game folder wasn’t remembered
import-fetching = Fetching { $pool } wishes, page { $page }…
import-fetched = { $count ->
        [one] { $count } new wish so far
       *[other] { $count } new wishes so far
    }
import-connecting = Connecting to the game server…
import-pace = The game server allows about one page of 20 wishes per second, so a long history takes a minute or two.
import-none-returned = The game returned no wishes from the last six months.
import-up-to-date = No new wishes: your history is up to date.
import-new-wishes = { $count ->
        [one] { $count } new wish
       *[other] { $count } new wishes
    }: { $pools }
import-new-account = A new account
import-no-uid-yet = { $account } (no UID yet)
import-choose-target = This UID has no account here yet. Add its wishes to:
import-linked = { $count ->
        [one] { $count } wish
       *[other] { $count } wishes
    } from paimon.moe matched the game’s records, which are noted for future imports.
import-try-again = Try again

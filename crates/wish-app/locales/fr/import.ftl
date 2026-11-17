# L’import des vœux : un fichier (paimon.moe, un instantané, UIGF, un tableur), ou
# l’historique derrière le lien du jeu.

import-file-button = Importer un fichier…
import-game-button = Importer depuis le jeu…
import-prompt = Importer
import-file-picker-failed = Impossible d’ouvrir le sélecteur de fichier
import-read-failed = Impossible de lire { $name }
import-title = Importer { $name } ?
import-button = Importer
import-game-title = Importer depuis le jeu
import-failed = Impossible d’importer { $name }
import-nothing-changed = Rien n’a été changé.
import-ok = OK

## Le compte rendu

import-nothing-new-game = Rien de nouveau : chaque vœu du jeu était déjà enregistré.
import-nothing-new-file = Rien de nouveau : chaque vœu de { $name } était déjà enregistré.
import-linked-only = Pas de nouveaux vœux. { $count ->
        [one] { $count } vœu enregistré correspondait
       *[other] { $count } vœux enregistrés correspondaient
    } aux données du jeu.
import-added = { $count ->
        [one] { $count } nouveau vœu ajouté.
       *[other] { $count } nouveaux vœux ajoutés.
    }
import-with-extras = { $headline } { $count ->
        [one] { $count } changement
       *[other] { $count } changements
    } dans vos personnages, armes, plan ou inventaire.

## L’aperçu d’un fichier

import-carried = Et { $list }
import-characters = { $count ->
        [one] { $count } personnage
       *[other] { $count } personnages
    }
import-weapons = { $count ->
        [one] { $count } arme
       *[other] { $count } armes
    }
import-plan-entries = { $count ->
        [one] { $count } entrée du plan
       *[other] { $count } entrées du plan
    }
import-materials = { $count ->
        [one] { $count } matériau
       *[other] { $count } matériaux
    }
import-check-match = La pity et les résultats des 50/50 correspondent à paimon.moe pour les { $count } valeurs qu’il a enregistrées.
import-check-differ = { $differ } valeurs sur { $checked } diffèrent de celles enregistrées par paimon.moe. Cela arrive quand les données des bannières de paimon.moe ont changé après votre dernier import là-bas. Vos vœux sont importés tels quels dans tous les cas.
import-preview-help = Les vœux déjà enregistrés restent inchangés, et une sauvegarde est faite d’abord. Les personnages et les armes sont ajoutés ou montés, jamais baissés ; les entrées du plan et les matériaux que vous avez déjà sont gardés tels quels.

## Importer depuis le jeu

import-game-help = Ouvrez l’historique des vœux dans le jeu (Vœux, puis Historique), puis trouvez son lien ici ou collez-le. Un lien marche environ un jour. Sur un téléphone ou une console, copiez le lien là-bas et collez-le ici.
import-find-link = Trouver le lien
import-choose-game-folder = Choisir le dossier du jeu…
import-choose-prompt = Choisir
import-fetch = Récupérer l’historique
import-link-found = Lien de votre dernière visite de l’historique des vœux trouvé. Si elle date de plus d’un jour, ouvrez d’abord à nouveau l’historique des vœux dans le jeu.
import-link-not-found = Aucun lien trouvé. Ouvrez l’historique des vœux dans le jeu (Vœux, puis Historique), puis réessayez, ou choisissez le dossier du jeu.
import-folder-not-remembered = Le dossier du jeu n’a pas été retenu
import-fetching = Récupération des vœux { $pool }, page { $page }…
import-fetched = { $count ->
        [one] { $count } nouveau vœu pour l’instant
       *[other] { $count } nouveaux vœux pour l’instant
    }
import-connecting = Connexion au serveur du jeu…
import-pace = Le serveur du jeu permet environ une page de 20 vœux par seconde, un long historique prend donc une minute ou deux.
import-none-returned = Le jeu n’a renvoyé aucun vœu des six derniers mois.
import-up-to-date = Pas de nouveaux vœux : votre historique est à jour.
import-new-wishes = { $count ->
        [one] { $count } nouveau vœu
       *[other] { $count } nouveaux vœux
    } : { $pools }
import-new-account = Un nouveau compte
import-no-uid-yet = { $account } (pas encore d’UID)
import-choose-target = Cet UID n’a pas encore de compte ici. Ajouter ses vœux à :
import-linked = { $count ->
        [one] { $count } vœu de paimon.moe correspondait
       *[other] { $count } vœux de paimon.moe correspondaient
    } aux données du jeu, qui sont notées pour les prochains imports.
import-try-again = Réessayer
import-server-asia = Asie
import-server-america = Amérique
import-server-europe = Europe
import-server-tw-hk-mo = TW, HK, MO
import-server-china = Chine

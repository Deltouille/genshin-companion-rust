# Les textes du cœur : les erreurs et les notes qu’il écrit pour le joueur.
# Une erreur suit ce qui a échoué et un deux-points (« Impossible d’importer : … »),
# elle commence donc par une minuscule.
# Typographie française : une espace insécable (U+00A0) avant ? ! : ; et à l’intérieur des « ».

## Erreurs

error-database = erreur de la base de données : { $detail }
error-io-reading = impossible de lire { $path } : { $detail }
error-io-writing = impossible d’écrire { $path } : { $detail }
error-io-creating = impossible de créer { $path } : { $detail }
error-io-copying = impossible de copier { $path } : { $detail }
error-io-copying-data-folder = impossible de copier le dossier des données vers { $path } : { $detail }
error-io-deleting = impossible de supprimer { $path } : { $detail }
error-io-flushing = impossible d’enregistrer { $path } sur le disque : { $detail }
error-io-listing = impossible de lister { $path } : { $detail }
error-io-locking = impossible de verrouiller { $path } : { $detail }
error-io-opening = impossible d’ouvrir { $path } : { $detail }
error-io-removing = impossible de retirer { $path } : { $detail }
error-io-renaming = impossible de renommer { $path } : { $detail }
error-io-replacing = impossible de remplacer { $path } : { $detail }
error-json = JSON invalide : { $detail }
error-no-data-dir = impossible de trouver un dossier de données pour cet utilisateur
error-schema-too-new = une version plus récente de Genshin Companion a écrit cette base (schéma { $found } ; celle-ci lit jusqu’au { $known }). Elle n’est pas touchée : mettez l’app à jour pour l’ouvrir
error-game-data = données du jeu invalides : { $detail }
error-not-paimon-export = ce n’est pas une sauvegarde paimon.moe : elle ne contient aucun historique de vœux
error-unrecognized-file = ce fichier n’est ni une sauvegarde paimon.moe (Settings → Export, ou le fichier Google Drive) ni un instantané Genshin Companion
error-unknown-items = le fichier contient des objets que cette version de Genshin Companion ne connaît pas encore : { $items }. Mettez l’app à jour, puis importez à nouveau le fichier
error-invalid-pull = `{ $key }` vœu n° { $index } : { $detail }
error-invalid-entry = `{ $key }` : { $detail }
error-not-snapshot = ce n’est pas un instantané Genshin Companion (format « { $found } », et non « { $expected } »)
error-snapshot-too-new = une version plus récente de Genshin Companion a écrit cet instantané (format { $found } ; celle-ci lit jusqu’au { $known }) : mettez l’app à jour pour l’importer
error-not-a-backup = { $file } n’est pas une sauvegarde de la base de cette app
error-damaged-backup = { $file } est endommagé ({ $problem }) ; choisissez une autre sauvegarde
error-excel = impossible de lire ou d’écrire le tableur : { $detail }
error-not-paimon-excel = ce tableur n’est pas un export de l’historique des vœux de paimon.moe
error-unknown-account = le compte { $account } n’existe pas
error-empty-account-name = un compte a besoin d’un nom
error-invalid-uid = « { $uid } » n’est pas un UID : un UID a 9 ou 10 chiffres
error-uid-taken = l’UID { $uid } est déjà le compte « { $account } »
error-account-mismatch = « { $account } » appartient déjà à un autre UID, les vœux de l’UID { $uid } ne peuvent donc pas y être ajoutés
error-not-manual = le vœu { $wish } n’a pas été saisi à la main, il ne peut donc pas être supprimé ; annulez plutôt son import
error-unknown-import = l’import { $import } n’existe pas
error-already-undone = l’import { $import } a déjà été annulé
error-invalid-link = ce n’est pas un lien d’historique des vœux. Copiez le lien entier : il commence par https:// et contient « authkey »
error-link-expired = le lien de l’historique des vœux a expiré. Ouvrez à nouveau l’historique des vœux dans le jeu, puis utilisez le nouveau lien
error-link-rejected = le serveur du jeu n’a pas accepté ce lien. Copiez à nouveau le lien entier ; si cela échoue encore, ouvrez à nouveau l’historique des vœux dans le jeu pour en obtenir un nouveau
error-api = le serveur du jeu a refusé la requête : { $message } ({ $retcode })
error-network = impossible de joindre le serveur du jeu : { $detail }
error-unexpected-response = réponse inattendue du serveur du jeu : { $detail }
error-image-download = impossible de télécharger l’image { $url } : { $problem }
error-mixed-accounts = l’historique mélange deux comptes (UID { $first } et { $second }) ; utilisez un lien d’un seul compte
error-cancelled = annulé

## Les erreurs de HoYoLAB : des phrases entières, montrées seules.

hoyolab-missing-cookie = Il manque { $field } au cookie. Copiez l’en-tête Cookie entier d’une requête vers hoyolab.com.
hoyolab-invalid-cookie = HoYoLAB n’accepte pas ce cookie. Reconnectez-vous à hoyolab.com, puis copiez-en un nouveau.
hoyolab-data-not-public = HoYoLAB garde ces données privées. Sur hoyolab.com, ouvrez les réglages du Journal de combat de votre profil et rendez-le public, puis réessayez.
hoyolab-captcha = HoYoLAB demande un captcha. Ouvrez une fois votre Journal de combat sur hoyolab.com, résolvez-le là-bas, puis réessayez.
hoyolab-too-frequent = HoYoLAB dit que les requêtes sont trop fréquentes. Attendez une minute, puis réessayez.
hoyolab-no-game-account = Aucun compte Genshin Impact n’est lié à ce compte HoYoLAB.
hoyolab-not-hoyoverse = { $host } n’est pas une des adresses de HoYoverse : le cookie n’est envoyé qu’à celles-ci.
hoyolab-api = HoYoLAB a répondu [{ $retcode }] { $message }
hoyolab-network = Impossible de joindre HoYoLAB : { $detail }
hoyolab-format = La réponse de HoYoLAB est illisible : { $detail }

## settings.json

settings-unreadable = settings.json n’a pas pu être lu ({ $error }) ; les réglages par défaut sont utilisés
settings-set-aside = settings.json n’a pas pu être lu ({ $error }) ; il a été gardé sous le nom { $aside } et les réglages par défaut sont utilisés
settings-not-set-aside = settings.json n’a pas pu être lu ({ $error }), ni mis de côté ({ $problem })

## Types de bannière

pool-beginners = Vœux des débutants
pool-standard = Standard
pool-character-event = Événement de personnage
pool-weapon-event = Événement d’arme
pool-chronicled = Vœux chroniques

# Les mêmes, là où ils sont rangés sous « Vœux » (la barre latérale), donc plus courts.
pool-short-beginners = Débutants
pool-short-standard = Standard
pool-short-character-event = Personnage
pool-short-weapon-event = Arme
pool-short-chronicled = Chroniques

## Imports

import-paimon-old-format = enregistré dans l’ancien format de paimon.moe ; ouvrez une fois le compteur de vœux sur paimon.moe pour qu’il convertisse les données, puis exportez à nouveau
import-paimon-out-of-order = { $account }, { $pool } : { $count ->
        [one] { $count } vœu n’est pas
       *[other] { $count } vœux ne sont pas
    } dans l’ordre chronologique dans les données de paimon.moe ; ils sont triés par date ici, ce qui peut changer leur pity
import-paimon-foreign-codes = { $account }, { $pool } : { $count ->
        [one] { $count } vœu portait
       *[other] { $count } vœux portaient
    } le type de vœu d’une autre bannière ; enregistré comme { $code }
import-held-back = { $count ->
        [one] { $count } vœu plus ancien de { $pool } n’a pas été ajouté
       *[other] { $count } vœux plus anciens de { $pool } n’ont pas été ajoutés
    } : l’historique de cette bannière contient des vœux saisis à la main dans paimon.moe, qu’ils pourraient répéter.
import-unknown-banner-type = { $count ->
        [one] { $count } vœu d’un type de bannière inconnu ({ $code }) laissé de côté
       *[other] { $count } vœux d’un type de bannière inconnu ({ $code }) laissés de côté
    } ; mettez l’app à jour.
import-conflicts = { $account }, { $pool } : { $count ->
        [one] { $count } vœu diffère
       *[other] { $count } vœux diffèrent
    } du vœu enregistré avec le même identifiant ; la version enregistrée a été gardée
import-counters-kept = { $account }, { $pool } : les compteurs manuels enregistrés (total { $stored_total }, pity 5★ { $stored_five }, pity 4★ { $stored_four }) ont été gardés à la place de ceux importés (total { $total }, pity 5★ { $five }, pity 4★ { $four })

## Synchronisation HoYoLAB : ce que le joueur doit savoir

sync-phase-unknown = { $name } est niveau { $level } : HoYoLAB ne dit pas si l’élévation a été faite, elle est donc comptée comme pas encore faite. Indiquez-la sur sa page si c’est le cas.
sync-above-max = { $name } est niveau { $level } : compté comme 90/90, le plus haut que cette version connaît.
sync-unknown-character = { $name } n’est pas dans les données du jeu de cette version : laissé de côté, avec son arme. Mettez l’app à jour pour le synchroniser.
sync-unknown-weapon = L’arme { $name } n’est pas dans les données du jeu de cette version : laissée de côté. Mettez l’app à jour pour la synchroniser.
sync-talents-unread = Les aptitudes de { $name } n’ont pas pu être lues : laissées telles quelles. Indiquez-les sur sa page.

## Le planificateur : le rôle de chaque matériau d’un personnage ou d’une arme personnalisés

role-common = Commun
role-local-specialty = Spécialité locale
role-boss-material = Matériau de boss
role-elemental-gem = Gemme élémentaire
role-talent-book = Livre d’aptitude
role-weekly-material = Matériau hebdomadaire
role-weapon-material = Matériau d’arme
role-elite-drop = Butin d’élite

## Les sources de l’estimation de résine, et où chacune se récolte

resin-talent-books = Livres d’aptitude
resin-weapon-materials = Matériaux d’arme
resin-boss-materials = Matériaux de boss
resin-character-exp = EXP de personnage
resin-mora = Moras
resin-place-talent-books = Domaines de la maîtrise
resin-place-weapon-materials = Domaines de la forge
resin-place-boss-materials = Boss normaux
resin-place-character-exp = Floraisons de la révélation
resin-place-mora = Floraisons de la richesse

## Les valeurs des aptitudes, comme le jeu les écrit

archive-percent = { $value }{"\u202F"}%

## Les matériaux « ? » du planificateur, pour un matériau pas encore connu

placeholder-common = Butin commun inconnu
placeholder-local-specialty = Spécialité locale inconnue
placeholder-boss-material = Matériau de boss inconnu
placeholder-elemental-gem = Gemme inconnue
placeholder-talent-book = Livre d’aptitude inconnu
placeholder-weekly-material = Matériau de boss hebdomadaire inconnu
placeholder-weapon-material = Matériau d’arme inconnu
placeholder-elite-drop = Butin d’élite inconnu
# Un niveau d’une série : « Livre d’aptitude inconnu (2★) ».
placeholder-tier = { $name } ({ $stars }★)

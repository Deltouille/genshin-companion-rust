# Paramètres : vos données, leurs sauvegardes et leur miroir, les imports récents,
# l’export, le comptage à la main, les données du jeu, les images et la langue.

settings-title = Paramètres
settings-save-failed = Impossible d’enregistrer le réglage

## Langue

settings-group-language = Langue
settings-language = Langue de l’interface
settings-language-help = Les textes de l’app, et les noms et descriptions du jeu.
# La langue du système, nommée dans elle-même : « Système (English) ».
settings-language-system = Système ({ $language })

## Vos données

settings-group-data = Vos données
settings-data-folder = Dossier des données
settings-data-folder-help = Votre base de vœux, ses sauvegardes et une copie de chaque fichier importé.
settings-reveal = Afficher dans le gestionnaire de fichiers
settings-backups = Sauvegardes
settings-backups-summary = { $count ->
        [one] { $count } sauvegarde
       *[other] { $count } sauvegardes
    }, la dernière le { $latest }. Faites avant chaque import, annulation ou restauration, et une fois par jour (les { $kept } plus récentes sont gardées), puis au plus une fois par heure pendant le comptage à la main, la modification du plan ou la chronique gardée toute seule (les { $kept_hourly } plus récentes).
settings-backups-none = Aucune sauvegarde pour l’instant. Une est faite avant chaque import ou annulation, et une fois par jour.
settings-back-up-now = Sauvegarder maintenant
settings-restore = Restaurer…
settings-mirror-folder = Dossier miroir
settings-mirror-help = Une copie de tous vos vœux, personnages, armes, plan, inventaire et chronique, mise à jour quelques secondes après chaque changement et à la fermeture de l’app. Choisissez un dossier synchronisé par Syncthing, Nextcloud, Dropbox ou un équivalent pour garder une copie hors de cet ordinateur.
settings-mirror-written = Écrit pour la dernière fois à { $time }.
settings-mirror-failed = Impossible d’écrire la dernière copie : { $error }
settings-mirror-pending = Écrit après le prochain changement.
settings-mirror-stop = Arrêter le miroir
settings-mirror-choose = Choisir un dossier…
settings-mirror-prompt = Copier ici
settings-mirror-change-failed = Impossible de changer le dossier miroir

## Sauvegardes

settings-backup-failed = Impossible de sauvegarder
settings-backup-saved = Sauvegarde faite.
settings-restore-title = Restaurer une sauvegarde
settings-restore-dialog-help = Restaurer remplace tout par le contenu de la sauvegarde : vos comptes et vos vœux, vos personnages et vos armes, et le plan et l’inventaire du planificateur. L’état actuel est sauvegardé d’abord.
settings-restore-choose-file = Choisir un fichier…
settings-restore-prompt = Restaurer
# Ce qui est restauré, dans une question : « Restaurer la sauvegarde du 9 nov. 2026, 12:20:05 ? »
settings-restore-what-backup = la sauvegarde du { $when }
settings-restore-confirm-title = Restaurer { $what } ?
settings-restore-confirm-help = Tout est remplacé par le contenu de la sauvegarde : vos comptes et vos vœux, vos personnages et vos armes, et le plan et l’inventaire du planificateur. L’état actuel est sauvegardé d’abord, pour pouvoir le restaurer au besoin.
settings-restore-button = Restaurer
settings-restore-failed = Impossible de restaurer
settings-restore-done = Sauvegarde restaurée. L’état précédent a été sauvegardé.

settings-backup-reason-daily = Sauvegarde du jour
settings-backup-reason-import = Avant un import
settings-backup-reason-migration = Avant une mise à jour de l’app
settings-backup-reason-manual = Sauvegardé à la main
settings-backup-reason-undo = Avant l’annulation d’un import
settings-backup-reason-restore = Avant la restauration d’une sauvegarde
settings-backup-reason-delete = Avant la suppression de vœux
settings-backup-reason-counting = Avant un comptage de vœux
settings-backup-reason-planning = Avant un changement de votre collection ou de votre plan
settings-backup-reason-sync = Avant une synchronisation HoYoLAB
settings-backup-reason-chronicle = Avant de garder la chronique
settings-backup-reason-account = Avant de supprimer un compte
settings-backup-reason-unknown = Sauvegarde

## Imports récents

settings-group-imports = Imports récents
settings-imports-none = Aucun import pour l’instant
settings-import-undone = annulé
settings-import-added = { $count ->
        [one] { $count } ajouté
       *[other] { $count } ajoutés
    }
settings-undo = Annuler…
settings-origin-game = Le jeu
settings-origin-manual = Compté à la main
settings-origin-manual-what = Compté à la main : { $what }
settings-origin-file = Un fichier
settings-undo-game-title = Annuler l’import depuis le jeu du { $time } ?
settings-undo-manual-title = Annuler { $what }, compté à la main ?
settings-undo-manual-wishes = les vœux
settings-undo-file-title = Annuler l’import de { $origin } ?
settings-undo-help = Cela retire { $count ->
        [one] le vœu qu’il a ajouté
       *[other] les { $count } vœux qu’il a ajoutés
    } à { $account }. Une sauvegarde est faite d’abord.
settings-undo-button = Annuler l’import
settings-undo-failed = Impossible d’annuler l’import
settings-undo-done = { $count ->
        [one] { $count } vœu retiré.
       *[other] { $count } vœux retirés.
    }

## Export

settings-group-export = Export
settings-export = Exporter…
settings-export-failed = Impossible d’exporter
settings-export-snapshot = Instantané
settings-export-snapshot-help = Tous les comptes, sans rien laisser de côté : vœux, personnages, armes, plan, inventaire et chronique. À réimporter ici, sur cet ordinateur ou un autre.
settings-export-uigf = UIGF
settings-export-uigf-help = Pour d’autres outils de suivi comme Snap Hutao ou Starward. Les comptes doivent avoir un UID.
settings-export-spreadsheet = Tableur
settings-export-spreadsheet-help = Le compte affiché, dans la présentation Excel de paimon.moe.

## Comptage

settings-group-counting = Comptage
settings-manual-counting = Compter les vœux à la main
settings-manual-counting-help = Ajoute +1, +10 et « Ajouter un 4★ ou un 5★… » à chaque page de bannière, comme le mode manuel de paimon.moe. Chaque ajout apparaît dans Imports récents et peut y être annulé.

## Données du jeu

settings-group-game-data = Données du jeu
settings-banners-items = Bannières et objets
settings-banners-items-help = Servent à retrouver les bannières et les résultats des vedettes.
settings-banners-items-source = Servent à retrouver les bannières et les résultats des vedettes. De paimon.moe, commit { $commit }.
settings-game-data-folder = Votre dossier de données du jeu
settings-game-data-partial = Tout n’est pas utilisé : { $warning }.

## Images

settings-group-images = Images
settings-images-downloaded = Images téléchargées
settings-images-help = { $count ->
        [one] { $count } image
       *[other] { $count } images
    }, { $size }. Les pages téléchargent une fois les images qu’elles montrent, depuis paimon.moe et Project Amber ; elles restent sur cet ordinateur.
settings-images-empty = Vider…
settings-images-offline = Pour une utilisation hors ligne
settings-images-offline-help = Téléchargez-les maintenant, pour que chaque page ait ses images sans connexion. Les icônes couvrent le compteur de vœux et le planificateur ; tout ajoute les illustrations des bannières et des personnages.
settings-images-stopping = Arrêt…
settings-images-downloading = Téléchargement de { $done } sur { $total }…
settings-images-progress = Images téléchargées
# `missing` : les images qu’aucun des deux sites n’a.
settings-images-done = { $missing ->
        [0] Toutes les images sont téléchargées.
        [one] Toutes les images sont téléchargées ({ $missing } absente de paimon.moe et de Project Amber).
       *[other] Toutes les images sont téléchargées ({ $missing } absentes de paimon.moe et de Project Amber).
    }
settings-images-failed = { $failed ->
        [one] { $failed } image n’a pas pu être téléchargée
       *[other] { $failed } images n’ont pas pu être téléchargées
    }{ $missing ->
        [0] {""}
        [one] {" "}({ $missing } absente de paimon.moe et de Project Amber)
       *[other] {" "}({ $missing } absentes de paimon.moe et de Project Amber)
    } : réessayez plus tard.
settings-images-stopped = Arrêté : { $stored } sur { $total } téléchargées.
settings-images-unreachable = Impossible de joindre paimon.moe ou Project Amber. Vérifiez la connexion, puis réessayez.
settings-images-icons = Icônes
settings-images-everything = Tout
# Un bouton de téléchargement quand il ne reste rien : « Icônes : fait ».
settings-images-offer-done = { $what } : fait
# Un bouton de téléchargement : « Icônes (~25 Mo) ».
settings-images-offer-size = { $what } (~{ $size })
settings-images-stop = Arrêter
settings-images-empty-title = Vider les images téléchargées ?
settings-images-empty-help = Elles sont téléchargées à nouveau quand les pages les montrent. Aucune de vos données n’est touchée.
settings-images-empty-button = Vider
settings-images-empty-failed = Impossible de vider les images téléchargées

## Comptes
settings-group-accounts = Comptes
settings-account-uid = UID { $uid }
settings-account-no-uid = Sans UID
settings-account-no-server = serveur inconnu
settings-account-wishes = { $count ->
        [0] aucun vœu
        [one] { $count } vœu
       *[other] { $count } vœux
    }
settings-account-characters = { $count ->
        [0] aucun personnage
        [one] { $count } personnage
       *[other] { $count } personnages
    }
settings-account-edit = Modifier…
settings-account-remove = Supprimer…
settings-account-add-title = Ajouter un compte
settings-account-add-help = Pour un joueur qui n’importe pas encore de vœux, ou un autre de vos comptes. Les imports et HoYoLAB créent leurs comptes d’eux-mêmes.
settings-account-add = Ajouter…
settings-account-add-button = Ajouter
settings-account-added = { $account } ajouté
settings-account-add-failed = Impossible d’ajouter le compte
settings-account-edit-title = Modifier le compte
settings-account-save = Enregistrer
settings-account-edit-failed = Impossible de modifier le compte
settings-account-remove-title = Supprimer { $account } ?
settings-account-remove-help = Ses personnages, ses armes, son plan et son inventaire partent avec lui. Une sauvegarde est faite d’abord : Restaurer… les ramène.
settings-account-remove-button = Supprimer
settings-account-remove-failed = Impossible de supprimer le compte
settings-account-removed = { $account } supprimé
settings-account-name = Nom
settings-account-uid-label = UID (facultatif)
settings-account-uid-placeholder = 9 ou 10 chiffres
settings-account-server = Serveur
settings-account-server-unknown = Non renseigné
settings-account-uid-kept = Il vient d’un import ou a été donné une fois : les imports retrouvent le compte par lui, il ne change donc plus.
settings-account-identity-help = Un UID dit son serveur. Sans UID, choisissez le serveur : les horaires des bannières et le jour du planificateur le suivent. Une fois donné, l’UID ne change plus.

## HoYoLAB : le cookie gardé dans le trousseau du système

settings-group-hoyolab = HoYoLAB
settings-hoyolab-kept = Cookie gardé sur cet ordinateur
settings-hoyolab-kept-help = Dans le trousseau de ce système (ltuid_v2, ltoken_v2 et ltmid_v2 seulement). La chronique de l’UID { $uids } se synchronise toute seule toutes les 6 heures pendant que l’app est ouverte, dans le compte qui a cet UID. Dernière synchronisation : { $last }.
settings-hoyolab-unlinked = Aucun compte ici n’a encore cet UID : synchronisez une fois avec HoYoLAB et choisissez le compte où il va.
settings-hoyolab-no-uid = aucun compte pour l’instant
settings-hoyolab-never = pas encore
settings-hoyolab-sync-now = Synchroniser maintenant
settings-hoyolab-forget = Oublier le cookie
settings-hoyolab-none = Aucun cookie gardé
settings-hoyolab-none-help = Cochez « Se souvenir de ce cookie sur cet ordinateur » lors d’une synchronisation avec HoYoLAB : la chronique se synchronise alors toute seule pendant que l’app est ouverte. Le cookie ne va que dans le trousseau de ce système.
settings-hoyolab-unavailable = Ce système n’a pas de trousseau que l’app puisse utiliser (sous Linux, un Secret Service comme GNOME Keyring ou KWallet), donc aucun cookie n’est gardé : chaque synchronisation le demande.
settings-hoyolab-checking = Interrogation du trousseau du système…

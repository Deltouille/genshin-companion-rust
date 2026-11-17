# Les textes de l’app : les menus, la coquille, ce que toutes les pages partagent.
# Typographie française : une espace insécable (U+00A0) avant ? ! : ; et à l’intérieur des « ».

app-menu-import-game = Importer depuis le jeu…
app-menu-import-file = Importer un fichier…
app-menu-sync-hoyolab = Synchroniser avec HoYoLAB…
app-menu-import-good = Importer un fichier GOOD…
app-menu-quit = Quitter

app-close = Fermer
app-cancel = Annuler

# Ce qui n’a pas eu lieu, puis pourquoi : « Impossible d’exporter : le disque est plein ».
app-failure = { $what } : { $why }
app-folder-picker-failed = Impossible d’ouvrir le sélecteur de dossier

## La barre latérale, et les onglets de la vue d’ensemble (les mêmes mots)

app-nav-overview = Vue d’ensemble
app-nav-wishes = Vœux
app-nav-characters = Personnages
app-nav-weapons = Armes
app-nav-resources = Ressources
app-nav-archive = Archives
app-nav-planner = Planificateur
app-nav-ascension = Élévation
app-nav-import = Importer
app-nav-settings = Paramètres
app-nav-credits = Crédits
app-account = Compte

## L’ouverture des données

app-opening = Ouverture de votre historique de vœux…
app-open-failed = Impossible d’ouvrir votre historique de vœux
app-open-failed-help = Vos données ne sont pas touchées. Elles sont dans { $folder }, avec des sauvegardes automatiques dans son dossier backups.
app-show-data-folder = Afficher le dossier des données
app-opening-database = ouverture de la base de vœux
app-not-open = vos données ne sont pas encore ouvertes

## Avant la fenêtre : l’app ne peut pas démarrer

app-cant-start = { $app } ne peut pas démarrer
app-error-file = Aussi écrit dans { $file }.
app-folder-failed = Le dossier des données n’a pas pu être préparé : { $error }.
app-already-open = { $app } est déjà ouvert. Une seule fenêtre à la fois peut utiliser votre dossier des données.

## Le titre de la fenêtre HoYoLAB

app-sync-hoyolab-title = Synchroniser avec HoYoLAB
app-import-good-title = Importer un fichier GOOD

## Écritures, exports et imports

app-write-panicked = une erreur interne l’a arrêté : { $detail }
app-write-panic = une panique
app-write-cut-off = le changement a été interrompu avant d’être enregistré
app-write-window-closed = la fenêtre s’est fermée avant que le changement soit enregistré
app-import-too-large = { $file } fait { $size }, trop pour un export d’historique de vœux
app-choose-account-first = choisissez d’abord un compte
app-exported = { $wishes ->
        [one] { $wishes } vœu exporté
       *[other] { $wishes } vœux exportés
    } de { $accounts ->
        [one] { $accounts } compte
       *[other] { $accounts } comptes
    }.
app-exported-left-out = { $wishes ->
        [one] { $wishes } vœu exporté
       *[other] { $wishes } vœux exportés
    } de { $accounts ->
        [one] { $accounts } compte
       *[other] { $accounts } comptes
    }. Laissés de côté, sans UID : { $left_out }.
app-exported-account = Vœux de { $account } exportés.
app-images-busy = des images sont en cours de téléchargement : arrêtez-le d’abord

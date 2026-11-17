# La page Importer, et la fenêtre de synchronisation HoYoLAB.

imports-title = Importer
imports-sync-button = Synchroniser avec HoYoLAB…
imports-wish-history = Historique des vœux
imports-wish-history-game = Depuis le jeu : l’app trouve le lien de l’historique des vœux dans le cache du jeu (Windows, ou un préfixe Wine ou Proton sous Linux), ou vous le collez depuis n’importe quel appareil, puis demande vos vœux au serveur de HoYoverse.
imports-wish-history-file = Depuis un fichier : une sauvegarde ou un tableur paimon.moe, un fichier UIGF v4 d’un autre outil de suivi, ou un instantané de cette app.
imports-collection = Personnages et armes
imports-collection-help = Depuis HoYoLAB : vos personnages tels qu’ils sont dans le jeu (niveau, constellations, aptitudes) et l’arme de chacun, dans votre collection. Votre cookie HoYoLAB ne sert qu’à cette synchronisation, et n’est jamais enregistré.
imports-collection-safe = Elle ne fait qu’ajouter ou monter ce que vous avez indiqué, jamais baisser, et montre ce qui changerait avant de rien écrire.
imports-undo = Chaque import peut être annulé, et une sauvegarde est faite avant chacun : voir Paramètres.

## La fenêtre HoYoLAB : le cookie

hoyolab-intro = HoYoLAB montre vos personnages tels qu’ils sont dans le jeu : leur niveau, leurs constellations et leurs aptitudes, et l’arme de chacun. L’app les lit avec votre cookie HoYoLAB, pour cette synchronisation seulement : le cookie n’est jamais enregistré, et ne va qu’à HoYoverse. Rien n’est écrit avant que vous confirmiez.
hoyolab-warning = Ces valeurs permettent à qui les a d’utiliser votre compte HoYoLAB. Ne les donnez qu’à cette app : jamais à un site, un bot Discord ou une autre personne.
hoyolab-step-1 = 1. Connectez-vous à hoyolab.com dans votre navigateur.
hoyolab-step-2 = 2. Ouvrez ses outils de développement (F12), puis Stockage (Application dans Chrome ou Edge) → Cookies → https://www.hoyolab.com.
hoyolab-step-3 = 3. Copiez les valeurs de ltuid_v2, ltoken_v2 et ltmid_v2 dans les champs ci-dessous.
hoyolab-pasted = Cookie lu dans le presse-papiers ({ $names }) ; le presse-papiers a été vidé. Si votre système garde un historique du presse-papiers (Win+V, Klipper…), supprimez-y aussi l’entrée.
hoyolab-use-fields = Utiliser les champs
hoyolab-paste = Coller un cookie entier
hoyolab-fine-print = Taper les trois valeurs est plus sûr que les coller : un historique du presse-papiers garde ce qui est copié. HoYoLAB ne montre ni les armes de votre sac ni vos matériaux. Les personnages à un palier de niveau (20, 40 … 80) comptent comme pas encore élevés, puisque HoYoLAB ne le dit pas. HoYoLAB n’a pas d’API publique : ceci utilise celle de son site, que HoYoverse peut changer ou bloquer.
hoyolab-fetch = Récupérer depuis HoYoLAB
hoyolab-fill-fields = Remplissez les trois champs, ou collez le cookie entier.

## La fenêtre HoYoLAB : l’aperçu

hoyolab-asking = Demande de vos personnages à HoYoLAB…
hoyolab-account = { $nickname } · RA { $level }
hoyolab-choose-target = Aucun compte ici n’a encore l’UID { $uid }. Choisissez celui dont l’historique de vœux est celui de ce joueur : il prend l’UID, et le prochain import depuis le jeu y ira aussi.
hoyolab-heading = UID { $uid } · { $nickname }, { $into }
hoyolab-nothing-new = Votre collection a déjà tout cela.
hoyolab-sync = Synchroniser
hoyolab-try-again = Réessayer
hoyolab-synced = Les personnages de { $nickname } sont synchronisés avec HoYoLAB : { $count ->
        [one] { $count } changement
       *[other] { $count } changements
    }.
hoyolab-new-account = Un nouveau compte, { $name }
hoyolab-into = dans { $account }
hoyolab-into-new = dans un nouveau compte, { $name }
hoyolab-to-add = { $count } à ajouter
hoyolab-to-raise = { $count } à monter
hoyolab-to-move = { $count } à déplacer
hoyolab-characters = Personnages : { $parts }
hoyolab-weapons = Armes : { $parts }
hoyolab-nothing = Rien à changer.
hoyolab-character-line = { $name } · { $level } · C{ $constellation } · aptitudes { $talents }
hoyolab-was = { $after }  (avant : { $before })
hoyolab-weapon-line = { $weapon } · { $level } · R{ $refinement }
hoyolab-weapon-held = { $weapon } · { $level } · R{ $refinement } · portée par { $holder }
hoyolab-moved = { $weapon } · { $level } · R{ $refinement } : de { $from } à { $to }
hoyolab-nobody = personne

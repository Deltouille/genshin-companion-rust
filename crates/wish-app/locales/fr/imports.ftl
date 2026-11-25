# La page Importer, et la fenêtre de synchronisation HoYoLAB.

imports-title = Importer
imports-sync-button = Synchroniser avec HoYoLAB…
imports-wish-history = Historique des vœux
imports-wish-history-game = Depuis le jeu : l’app trouve le lien de l’historique des vœux dans le cache du jeu (Windows, ou un préfixe Wine ou Proton sous Linux), ou vous le collez depuis n’importe quel appareil, puis demande vos vœux au serveur de HoYoverse.
imports-wish-history-file = Depuis un fichier : une sauvegarde ou un tableur paimon.moe, un fichier UIGF v4 d’un autre outil de suivi, ou un instantané de cette app.
imports-collection = Personnages, armes et chronique
imports-collection-help = Depuis HoYoLAB : vos personnages tels qu’ils sont dans le jeu (niveau, constellations, aptitudes) et l’arme de chacun, dans votre collection ; et votre Abîme spiral, votre Théâtre imaginaire et votre Journal du voyageur, que HoYoLAB ne montre que pour un temps, dans la Chronique. Votre cookie HoYoLAB ne sert qu’à cette synchronisation, sauf si vous demandez à l’app de s’en souvenir dans le trousseau de votre système : la chronique se synchronise alors toute seule.
imports-collection-safe = Elle ne fait qu’ajouter ou monter ce que vous avez indiqué, jamais baisser, garde une période de la chronique une fois terminée, et montre ce qui changerait avant de rien écrire.
imports-good = Personnages, armes et matériaux
imports-good-help = Depuis un fichier GOOD, qu’exportent Irminsul, Inventory Kamera et Genshin Optimizer : vos personnages, toutes les armes de votre sac et vos matériaux, dans le compte affiché. Le planificateur compte alors avec vos vrais matériaux.
imports-good-safe = Les personnages et les armes sont seulement ajoutés ou montés, jamais baissés ; les matériaux prennent les quantités du fichier. Un aperçu montre chaque changement avant de rien écrire.
imports-good-button = Importer un fichier GOOD…
imports-undo = Chaque import peut être annulé, et une sauvegarde est faite avant chacun : voir Paramètres.

## La fenêtre HoYoLAB : le cookie

hoyolab-intro = HoYoLAB montre vos personnages tels qu’ils sont dans le jeu (leur niveau, leurs constellations et leurs aptitudes, et l’arme de chacun) et, pour un temps seulement, votre Abîme spiral, votre Théâtre imaginaire et votre Journal du voyageur, que l’app garde pour de bon. L’app les lit avec votre cookie HoYoLAB, qui ne va qu’à HoYoverse et ne sert que pour cette synchronisation, sauf si vous cochez « Se souvenir » plus bas. Rien n’est écrit avant que vous confirmiez.
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
hoyolab-remember = Se souvenir de ce cookie sur cet ordinateur
hoyolab-remember-help = Seuls ltuid_v2, ltoken_v2 et ltmid_v2 vont dans le trousseau de ce système, une fois que HoYoLAB les accepte. La chronique se synchronise alors toute seule pendant que l’app est ouverte ; Paramètres → HoYoLAB l’oublie.
hoyolab-remember-replaces = Un cookie est déjà gardé : celui-ci prend sa place.
hoyolab-remember-unavailable = Ce système n’a pas de trousseau que l’app puisse utiliser (sous Linux, un Secret Service comme GNOME Keyring ou KWallet) : le cookie ne sert que pour cette synchronisation.
hoyolab-remember-failed = Le cookie n’a pas été gardé

## La fenêtre HoYoLAB : l’aperçu

hoyolab-asking = Demande de vos personnages et de votre chronique à HoYoLAB…
hoyolab-account = { $nickname } · RA { $level }
hoyolab-choose-target = Aucun compte ici n’a encore l’UID { $uid }. Choisissez celui dont l’historique de vœux est celui de ce joueur : il prend l’UID, et le prochain import depuis le jeu y ira aussi.
hoyolab-heading = UID { $uid } · { $nickname }, { $into }
hoyolab-nothing-new = Votre collection et votre chronique ont déjà tout cela.
hoyolab-sync = Synchroniser
hoyolab-try-again = Réessayer
hoyolab-synced = Les personnages et la chronique de { $nickname } sont synchronisés avec HoYoLAB : { $count ->
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
hoyolab-chronicle-abyss = Abîme spiral : { $parts }
hoyolab-chronicle-theater = Théâtre imaginaire : { $parts }
hoyolab-chronicle-diary = Journal du voyageur : { $parts }
hoyolab-to-keep = { $count } à garder
hoyolab-to-update = { $count } à mettre à jour
hoyolab-chronicle-note = HoYoLAB ne montre ces périodes que pour un temps. Gardée ici, une période terminée ne change plus ; celle en cours est mise à jour à la synchronisation suivante.

## La fenêtre d’import GOOD

good-prompt = Choisissez un fichier GOOD
good-intro = Un fichier GOOD contient vos personnages, les armes de votre sac et vos matériaux. Il va dans le compte affiché, { $account } : pour l’importer dans un autre, choisissez-le d’abord dans la barre latérale.
good-intro-new = Un fichier GOOD contient vos personnages, les armes de votre sac et vos matériaux. Il va dans un nouveau compte, « Main ».
good-how-irminsul = Irminsul : lancez-le, puis le jeu ; une fois vos données lues, exportez-les dans un fichier ou dans le presse-papiers.
good-how-kamera = Inventory Kamera : scannez votre compte, puis choisissez le fichier qu’il écrit.
good-how-optimizer = Genshin Optimizer : exportez votre base de données, puis choisissez le fichier.
good-safe = Les personnages et les armes sont seulement ajoutés ou montés, jamais baissés. Les matériaux du planificateur prennent les quantités du fichier, ceux que le fichier n’a pas passent à 0, et les Moras restent tels quels. Rien n’est écrit avant votre confirmation, et une sauvegarde est faite d’abord.
good-paste = Coller depuis le presse-papiers
good-choose-file = Choisir un fichier…
good-clipboard-empty = Le presse-papiers ne contient pas de texte : copiez d’abord l’export GOOD, ou choisissez le fichier.
good-reading = Lecture du fichier…
good-heading = Depuis { $source }, { $into }
good-heading-unnamed = Depuis un fichier GOOD, { $into }
good-into = dans { $account }
good-into-new = dans un nouveau compte, « Main »
good-materials = Matériaux : { $count } à changer
good-section-characters = Personnages
good-section-weapons = Armes
good-section-materials = Matériaux
good-grouped = { $line } ×{ $count }
good-material-line = { $name } : { $before } → { $after }
good-nothing-new = Votre collection et votre inventaire ont déjà tout ceci.
good-import = Importer
good-imported = Le fichier GOOD est importé dans { $account } : { $count ->
        [one] { $count } changement
       *[other] { $count } changements
    }.

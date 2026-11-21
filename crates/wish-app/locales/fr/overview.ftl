# La vue d’ensemble, l’accueil de l’app : le compte en chiffres, un onglet pour
# ses vœux, ses personnages et ses armes.

overview-title = Vue d’ensemble
overview-uid = UID { $uid }
overview-no-wishes = Pas encore de vœux
overview-no-wishes-help = Importez votre historique de vœux depuis le jeu, ou vos données paimon.moe : dans paimon.moe, ouvrez Settings et choisissez Export, ou téléchargez le fichier de sauvegarde depuis votre Google Drive. Importez les deux pour garder l’historique que paimon.moe a d’avant les six derniers mois du jeu.
overview-no-wishes-manual = Ou ouvrez une bannière dans la barre latérale pour compter vos vœux à la main.
# Sous un chiffre : « sur 97 », ce par rapport à quoi il est compté.
overview-of = sur { $total }

## Onglet Personnages

overview-no-characters = Pas encore de personnages
overview-no-characters-help = Synchronisez-les avec HoYoLAB, ou indiquez les personnages que vous avez dans les archives : ils sont comptés ici, par rareté, élément, arme et constellation.
overview-open-characters = Ouvrir les personnages
overview-characters = Personnages
overview-level = Niveau { $level }
overview-crowns = { $count ->
        [one] { $count } aptitude
       *[other] { $count } aptitudes
    } au niveau { $level }
overview-by-element = Par élément
overview-by-weapon = Par arme
overview-by-region = Par région
# Les personnages sans région dans les données du jeu : le Voyageur, Aloy.
overview-region-none = Sans région
overview-by-version = Personnages par version
overview-released = Sortis
overview-owned = Possédés
overview-version = Version { $version }
overview-five-constellations = Constellations des 5★
overview-four-constellations = Constellations des 4★
overview-levels = Niveaux
overview-talents = Aptitudes
# Dans le trou d’un donut, sous le nombre.
overview-characters-word = { $count ->
        [one] personnage
       *[other] personnages
    }
overview-talents-word = { $count ->
        [one] aptitude
       *[other] aptitudes
    }
overview-talent-averages = Moyenne : { NUMBER($normal, maximumFractionDigits: 1) } en attaque normale, { NUMBER($skill, maximumFractionDigits: 1) } en compétence, { NUMBER($burst, maximumFractionDigits: 1) } en déchaînement
overview-invested = Ressources investies
overview-invested-help = Ce qu’a coûté la montée de vos personnages jusqu’où ils en sont, depuis le niveau 1 avec toutes les aptitudes au niveau 1, et la résine que cela demande en moyenne.
overview-invested-levels = Pour les niveaux
overview-invested-talents = Pour les aptitudes
overview-talent-books = Livres d’aptitude
overview-every-tier = Tous niveaux confondus
overview-weekly-materials = Matériaux de boss hebdomadaires
overview-resin = Résine

## Onglet Armes

overview-no-weapons = Pas encore d’armes
overview-no-weapons-help = Indiquez les armes que vous avez dans les archives, ou synchronisez-les avec HoYoLAB avec les personnages qui les portent : elles sont comptées ici, par rareté et par type.
overview-open-weapons = Ouvrir les armes
overview-weapons = Armes
overview-refined-fully = Raffinées R5
overview-copies-held = { $held } { $copies ->
        [one] exemplaire porté sur { $copies }
       *[other] exemplaires portés sur { $copies }
    }
overview-by-type = Par type
overview-five-star-weapons = Armes 5★
overview-four-star-weapons = Armes 4★
overview-lesser-weapons = Et { $count ->
        [one] { $count } arme
       *[other] { $count } armes
    } de 3★ ou moins, dans les archives.
overview-copies = { $count } exemplaires

## Onglet Vœux

overview-wishes = Vœux
overview-primogems = { $count } primo-gemmes
overview-average-pity = Pity moyenne { $pity }
overview-none-yet = Aucun pour l’instant
overview-rate-won = 50/50 gagnés
overview-won-lost = { $won } { $won ->
        [one] gagné
       *[other] gagnés
    } · { $lost } { $lost ->
        [one] perdu
       *[other] perdus
    }
overview-guaranteed = Garanti
overview-guaranteed-at = Garanti à { $pity }
overview-five-pity = Pity 5★
overview-four-pity = Pity 4★
overview-history = Historique de { $pool }
overview-wish-count = { $count ->
        [one] { $count } vœu
       *[other] { $count } vœux
    }
overview-outside-banners = { $count ->
        [one] { $count } vœu tombe
       *[other] { $count } vœux tombent
    } hors de toutes les bannières que cette version connaît : mettez l’app à jour.
overview-unknown-items = { $count ->
        [one] { $count } vœu contient
       *[other] { $count } vœux contiennent
    } un objet que cette version ne connaît pas.
overview-manual-counters = Compteurs saisis à la main dans paimon.moe.
overview-total = Total
overview-share = Part
overview-average = Pity moy.
overview-character = Personnage
overview-weapon = Arme
# Une ligne du tableau 5★/4★, sous sa rareté : les 50/50 (ou 75/25) gagnés.
# Court, la colonne est étroite ; l’étiquette au-dessus dit 50/50 ou 75/25.
overview-won-odds = Gagnés{ $odds ->
       *[other] {""}
    }
overview-no-five = Pas encore de 5★.
overview-every-five = Chaque 5★ et sa pity, le plus récent d’abord
overview-pill-won = { $odds } gagné
overview-pill-lost = { $odds } perdu
overview-pill-guaranteed = garanti
overview-per-month = Vœux par mois
overview-one-month = { $count ->
        [one] Votre { $count } vœu a été fait
       *[other] Vos { $count } vœux ont tous été faits
    } en { $month } : { $four } × 4★, { $five } × 5★.
overview-month-wishes = { $count } ({ $primogems } primo-gemmes)

# Le planificateur d’élévation : les cartes du plan, ce qu’il manque encore,
# l’ajout d’un personnage, d’une arme ou d’un personnalisé, l’inventaire et
# l’estimation de résine.

planner-title = Planificateur d’élévation
planner-subtitle = Ce que coûte de faire monter vos personnages et vos armes
planner-add-character = Ajouter un personnage…
planner-add-weapon = Ajouter une arme…
planner-custom = Personnalisé…
planner-custom-tooltip = Pour un personnage ou une arme que les données du jeu n’ont pas encore : un nom et ses matériaux
planner-empty = Rien de prévu pour l’instant
planner-empty-help = Ajoutez un personnage ou une arme pour voir ce que coûte de le faire monter, ou un personnalisé que les données du jeu n’ont pas encore.
planner-not-saved = Le plan n’a pas été enregistré
planner-remove-title = Retirer { $name } du plan ?
planner-remove-help = Son objectif est oublié. Une sauvegarde est faite d’abord.
planner-remove = Retirer
planner-remove-tooltip = Retirer { $name } du plan
planner-edit = Modifier { $name }
planner-drag = Glissez pour changer l’ordre : les premiers personnages se servent d’abord dans votre inventaire

## Une carte

planner-level = Niveau
planner-level-of = Niveau de { $name }
planner-talent-of = { $talent } de { $name }
planner-normal-attack = Attaque normale
planner-elemental-skill = Compétence élémentaire
planner-elemental-burst = Déchaînement élémentaire
planner-character-gone = Ce personnage n’est plus dans les données du jeu.
planner-weapon-gone = Cette arme n’est plus dans les données du jeu.
planner-custom-character = Personnage personnalisé · { $stars }★
planner-custom-weapon = Arme personnalisée · { $stars }★
planner-custom-entry = Entrée personnalisée
planner-nothing-to-spend = Rien à dépenser.

## L’infobulle d’un matériau

planner-tip-covered = { $name } : { $need } { $need ->
        [one] nécessaire
       *[other] nécessaires
    }, couverts par votre inventaire
planner-tip-partly = { $name } : { $need } { $need ->
        [one] nécessaire
       *[other] nécessaires
    }, encore { $lack } à obtenir
planner-tip-needed = { $name } : { $need } { $need ->
        [one] nécessaire
       *[other] nécessaires
    }
planner-tip-plan = { $name } : { $need } pour tout le plan, { $owned } en stock, encore { $lack } à obtenir
planner-tip-crafted = { $tip } ; { $crafted } { $crafted ->
        [one] fabriqué
       *[other] fabriqués
    } à partir des niveaux inférieurs
planner-tip-unknown = { $tip } (absent des données du jeu)

## Encore nécessaire

planner-still-needed = Encore nécessaire
planner-entries = { $count ->
        [one] { $count } entrée
       *[other] { $count } entrées
    }
planner-still-needed-help = Ce qui manque au plan une fois votre inventaire réparti, dans l’ordre du plan ; les niveaux inférieurs sont fabriqués vers le haut. Cliquez sur un matériau pour indiquer combien vous en avez.

## Ajouter un personnage ou une arme

planner-search-character = Chercher un personnage
planner-search-weapon = Chercher une arme
planner-owned-level = Niv. { $level } · C{ $constellation }
planner-planned = Prévu
planner-owned = Possédés
planner-not-owned = Non possédés
planner-every-weapon = Toutes les armes
planner-section-count = { $title } · { $count }
planner-no-match = Rien ne correspond à cette recherche.
planner-add-character-title = Ajouter un personnage
planner-add-weapon-title = Ajouter une arme

## Un personnage ou une arme personnalisés

planner-name = Nom
planner-search = Rechercher…
planner-type-name = Pas dans la liste ? Tapez son nom
planner-type-name-first = Tapez d’abord le nom du matériau.
planner-name-character = Donnez un nom au personnage.
planner-name-weapon = Donnez un nom à l’arme.
# Ce qui manque encore au formulaire, par rôle.
planner-choose-its = { $role ->
        [common] Choisissez son butin commun.
        [local-specialty] Choisissez sa spécialité locale.
        [boss-material] Choisissez son matériau de boss.
        [elemental-gem] Choisissez sa gemme élémentaire.
        [talent-book] Choisissez son livre d’aptitude.
        [weekly-material] Choisissez son matériau hebdomadaire.
        [weapon-material] Choisissez son matériau d’arme.
       *[elite-drop] Choisissez son butin d’élite.
    }
# Le titre du choix d’un rôle.
planner-choose-the = { $role ->
        [common] Choisir le butin commun
        [local-specialty] Choisir la spécialité locale
        [boss-material] Choisir le matériau de boss
        [elemental-gem] Choisir la gemme élémentaire
        [talent-book] Choisir le livre d’aptitude
        [weekly-material] Choisir le matériau hebdomadaire
        [weapon-material] Choisir le matériau d’arme
       *[elite-drop] Choisir le butin d’élite
    }
planner-character = Personnage
planner-weapon = Arme
planner-choose = Choisir…
planner-stars = Étoiles
planner-back = Retour
planner-nothing-in-data = Rien ne correspond dans les données du jeu.
planner-use-name = Utiliser ce nom
planner-edit-weapon-title = Modifier l’arme personnalisée
planner-edit-character-title = Modifier le personnage personnalisé
planner-add-custom-title = Ajouter un personnage ou une arme personnalisés
planner-save = Enregistrer
planner-add = Ajouter

## L’inventaire

planner-inventory = Inventaire
planner-change-hint = +60 000 ou -20
planner-fewer-than-none = Vous ne pouvez pas en avoir moins que zéro.
planner-whole-numbers = Tapez des nombres entiers, comme 360 000 ou -20.
planner-nothing-still-needed = plus rien de nécessaire
planner-n-still-needed = encore { $count } { $count ->
        [one] nécessaire
       *[other] nécessaires
    }
planner-plan-needs-line = Le plan en demande { $count }, { $still }
planner-you-have = Vous avez
planner-add-or-subtract = Ajouter ou retirer
planner-ley-line = Ajouter une floraison (+{ $mora })
planner-exp = { $exp } EXP
# Livres d’EXP : « 4 Expérience de l’aventurier ou 20 Conseils du vagabond font le travail d’une Expérience du héros. »
planner-exp-note = Chacun compte pour son EXP : { $others } font le travail d’un { $top }.
planner-or = { $first } ou { $second }
planner-craft-note = Ce qui manque à un niveau est fabriqué avec le surplus des niveaux inférieurs, trois pour un : ce qui reste nécessaire est ce qu’il reste à récolter.
planner-plan-needs = Le plan en demande
planner-crafted = Fabriqués

## L’estimation de résine

planner-days = { NUMBER($days, minimumFractionDigits: 1, maximumFractionDigits: 1) } { $days ->
        [one] jour
       *[other] jours
    }
planner-runs = { $count ->
        [one] { $count } passage
       *[other] { $count } passages
    }
planner-resin-source = { $source } : { $runs }, { $resin } de résine
planner-resin = ~{ $resin } de résine
planner-resin-days = · ~{ $days }
planner-resin-count = { $resin } de résine
planner-resin-options = Options de résine…
planner-resin-nothing = Rien de ce qui manque encore ne se récolte avec de la résine.
planner-resin-no-source = Aucune source n’est comptée : choisissez-les dans les options.
planner-resin-about = Environ { $days } à { $per_day } de résine par jour, niveau du monde { $level }
planner-resin-per-day-error = La résine par jour est un nombre entier de 1 à { $most }.
planner-world-level = Niveau du monde
planner-world-level-help = Les boss donnent plus de matériaux quand il augmente ; les floraisons donnent le plus à partir de 6.
planner-resin-per-day = Résine par jour
planner-resin-per-day-help = 180 se régénèrent en un jour ; ajoutez ce que vous rechargez ou prenez à la Résine fragile.
planner-resin-sources = Compter
planner-resin-sources-help = Les domaines comptent à leur plus haut niveau. Les boss hebdomadaires, les gemmes, les minerais, les butins d’ennemis et les spécialités locales ne prennent pas de résine comptée ici.
planner-resin-title = Estimation de résine
planner-resin-not-saved = Les options de résine n’ont pas été enregistrées

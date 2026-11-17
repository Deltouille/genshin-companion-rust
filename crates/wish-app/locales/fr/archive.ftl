# Les archives : les personnages et les armes du jeu, leurs pages, et ce que le
# compte en possède.

## Les listes et leurs filtres

archive-sort-version = Version
archive-sort-name = Nom
archive-sort-rarity = Rareté
archive-filter = Filtrer
archive-filter-count = Filtrer · { $count }
archive-order-reversed = Ordre inversé
archive-reverse-order = Inverser l’ordre
archive-remove-filter = Retirer le filtre { $filter }
archive-sort-by = Trier par{" "}
archive-order = Ordre
archive-clear-all = Tout effacer
archive-close = Fermer
archive-done = Terminé
archive-filter-on = { $filter }, actif
archive-favourites = Favoris
archive-group-element = Élément
archive-group-weapon = Arme
archive-group-type = Type
archive-group-rarity = Rareté
archive-group-collection = Collection
archive-owned = Possédés
archive-not-owned = Non possédés
archive-new = Nouveau
archive-built-in-data = Données du jeu intégrées utilisées : { $warning }
archive-add-favourite = Ajouter aux favoris
archive-remove-favourite = Retirer des favoris
archive-remove = Retirer
archive-nothing-to-spend = Rien à dépenser.
archive-material = { $count } × { $name }
archive-owned-level = Niv. { $level } · C{ $constellation }
archive-level-cap = Niv. { $level }/{ $cap }
archive-level = Niveau
archive-to-reach = Pour atteindre { $level }
archive-from-level-one = depuis le niveau 1
archive-released-in = Sorti en
archive-add-to-planner = Ajouter au planificateur
archive-not-planned = { $name } n’a pas été ajouté au planificateur
archive-not-added = { $name } n’a pas été ajouté

## Personnages

archive-characters = Personnages
archive-search-character = Chercher un personnage
archive-characters-subtitle = { $owned } { $owned ->
        [one] possédé
       *[other] possédés
    } · { $total } personnages
archive-no-owned-character-match = Aucun de vos personnages ne correspond à ces filtres.
archive-owned-characters-help = Les personnages que vous avez s’affichent ici, avec leur niveau et leur constellation. Ajoutez-les avec + sur leur carte.
archive-no-character-match = Aucun personnage ne correspond à ces filtres.
archive-add-character = Ajouter à mes personnages
archive-character-label = { $name }, { $element } { $weapon } { $stars }★
archive-character-label-owned = { $name }, { $element } { $weapon } { $stars }★, possédé
archive-character = Personnage
archive-character-missing = Ce personnage n’est pas dans les données du jeu.
archive-all-characters = Tous les personnages
archive-stats = Statistiques
archive-ascension = Élévation
archive-talents = Aptitudes
archive-constellations = Constellations
archive-base-stats = Statistiques de base
archive-talent-level = Niveau d’aptitude
archive-talent-badge = Niv. { $level }
archive-talents-past-ten = Les niveaux 11 à 15 viennent des constellations ; les matériaux s’arrêtent à 10.
archive-talent-cost = Monter une aptitude de 1 à { $level } :
archive-fact-element = Élément
archive-fact-weapon = Arme
archive-fact-region = Région
archive-fact-affiliation = Affiliation
archive-fact-birthday = Anniversaire
archive-fact-constellation = Constellation
archive-character-planned = { $name } est dans le planificateur, du niveau { $level } à 90 avec les aptitudes à 10.
archive-character-owned = { $name } est dans vos personnages. Indiquez le niveau, la constellation et les aptitudes sur sa page.
archive-remove-character-title = Retirer { $name } de vos personnages ?
archive-remove-character-help = Le niveau, la constellation et les aptitudes indiqués sont oubliés ; l’arme portée reste dans vos armes. Une sauvegarde est faite d’abord.
archive-lower-talent = Baisser { $talent }
archive-raise-talent = Monter { $talent }
archive-held-weapon = { $name } · R{ $refinement } · { $level }/{ $cap }
archive-no-weapon-held = Aucune : donnez-lui en une sur la page d’une arme
archive-my-character = Mon personnage
archive-constellation = Constellation
archive-characters-not-saved = Vos personnages n’ont pas été enregistrés

## Armes

archive-weapons = Armes
archive-search-weapon = Chercher une arme
archive-weapons-subtitle = { $owned } { $owned ->
        [one] possédée
       *[other] possédées
    } · { $total } armes
archive-no-owned-weapon-match = Aucune de vos armes ne correspond à ces filtres.
archive-owned-weapons-help = Les armes que vous avez s’affichent ici, avec leur meilleur raffinement et leur nombre d’exemplaires. Ajoutez-les avec + sur leur carte.
archive-no-weapon-match = Aucune arme ne correspond à ces filtres.
archive-weapon-label = { $name }, { $kind } { $stars }★
archive-weapon-label-owned = { $name }, { $kind } { $stars }★, possédée
archive-weapon = Arme
archive-weapon-missing = Cette arme n’est pas dans les données du jeu.
archive-all-weapons = Toutes les armes
archive-fact-type = Type
archive-fact-secondary = Stat. secondaire
archive-fact-passive = Effet passif
archive-weapon-planned = { $name } est dans le planificateur, du niveau { $from } à { $to }.
archive-copy-added = Un exemplaire de { $name } est dans vos armes. Indiquez son niveau, son raffinement et qui la porte sur sa page.
archive-remove-copy-title = Retirer cet exemplaire de { $name } ?
archive-remove-copy-help = Son niveau, son raffinement et qui la porte sont oubliés. Une sauvegarde est faite d’abord.
archive-copy-not-removed = L’exemplaire n’a pas été retiré
archive-nobody = Personne
archive-i-have-it = Je l’ai
archive-add-copy = Ajouter un exemplaire
archive-refinement = Raffinement
archive-held-by = Portée par
archive-my-copies = Mes exemplaires
archive-no-copies = Vous ne l’avez pas. Ajoutez un exemplaire pour noter son niveau, son raffinement et qui la porte.
archive-weapons-not-saved = Vos armes n’ont pas été enregistrées

## Le type d’une aptitude

archive-kind-normal-attack = Attaque normale
archive-kind-elemental-skill = Compétence élémentaire
archive-kind-elemental-burst = Déchaînement élémentaire
archive-kind-alternate-sprint = Sprint alternatif
archive-kind-passive = Aptitude passive

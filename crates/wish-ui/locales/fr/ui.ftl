# Les textes du système de design : nombres, dates et les mots des composants.

## Nombres. `value` est déjà écrit avec les chiffres de la langue.
## Une espace fine insécable (U+202F) avant %, k, ko… comme le veut la typographie française.

ui-percent = { $value }{" "}%
ui-thousands = { $value }{" "}k
ui-millions = { $value }{" "}M
ui-kilobytes = { $value }{" "}ko
ui-megabytes = { $value }{" "}Mo
ui-gigabytes = { $value }{" "}Go

## Dates : `day` et `year` sont des nombres, `month` le nom d'un mois ci-dessous.

ui-date = { $day } { $month } { $year }
ui-date-time = { $date }, { $time }
ui-date-at-time = { $date } à { $time }
ui-month-year = { $month } { $year }

ui-month-short-1 = janv.
ui-month-short-2 = févr.
ui-month-short-3 = mars
ui-month-short-4 = avr.
ui-month-short-5 = mai
ui-month-short-6 = juin
ui-month-short-7 = juil.
ui-month-short-8 = août
ui-month-short-9 = sept.
ui-month-short-10 = oct.
ui-month-short-11 = nov.
ui-month-short-12 = déc.

ui-month-1 = janvier
ui-month-2 = février
ui-month-3 = mars
ui-month-4 = avril
ui-month-5 = mai
ui-month-6 = juin
ui-month-7 = juillet
ui-month-8 = août
ui-month-9 = septembre
ui-month-10 = octobre
ui-month-11 = novembre
ui-month-12 = décembre

## Le 50/50

ui-guaranteed-rate-up = vedette garantie
ui-outcome-won = Gagné
ui-outcome-lost = Perdu
ui-outcome-guaranteed = Garanti
# Les données de la bannière ne collent pas au vœu : n'arrive jamais dans le jeu.
ui-outcome-unexpected = Inattendu

## Guillemets, autour d’un nom donné par le joueur ou du nom d’un fichier

ui-quoted = « { $text } »

## Contrôles

ui-cancel = Annuler
ui-close = Fermer
ui-choose = Choisir…
ui-search = Rechercher
ui-nothing-matches = Aucun résultat.
# Un sélecteur lu à voix haute : son nom, puis son choix.
ui-picker-label = { $label } : { $choice }
ui-previous = Précédent
ui-next = Suivant
ui-page = Page { $page }
ui-pages = Pages { $first } à { $last }
# Un groupe de la barre latérale lu à voix haute.
ui-expanded = { $label }, déplié
ui-collapsed = { $label }, replié
ui-current-page = { $label }, page actuelle

## Le graphique de la pity 5★

ui-pity-title = Pity 5★
ui-pity-average = moyenne { $average }
ui-pity-one = Un seul 5★ pour l’instant, à la pity { $pity }.
ui-pity-none = Pas encore de 5★.
ui-pity = Pity

## Les éléments et les types d’arme du jeu, comme le jeu les nomme

ui-element-pyro = Pyro
ui-element-hydro = Hydro
ui-element-anemo = Anémo
ui-element-electro = Électro
ui-element-dendro = Dendro
ui-element-cryo = Cryo
ui-element-geo = Géo
ui-weapon-sword = Épée à une main
ui-weapon-claymore = Épée à deux mains
ui-weapon-polearm = Arme d’hast
ui-weapon-bow = Arc
ui-weapon-catalyst = Catalyseur

## Les statistiques, comme le jeu les nomme

ui-stat-hp = PV
ui-stat-atk = ATQ
ui-stat-def = DÉF
ui-stat-base-hp = PV de base
ui-stat-base-atk = ATQ de base
ui-stat-base-def = DÉF de base
ui-stat-crit-rate = Taux CRIT
ui-stat-crit-dmg = DGT CRIT
ui-stat-energy-recharge = Recharge d’énergie
ui-stat-elemental-mastery = Maîtrise élémentaire
ui-stat-healing-bonus = Bonus de soins
ui-stat-physical-dmg = Bonus de DGT physiques
ui-stat-pyro-dmg = Bonus de DGT Pyro
ui-stat-hydro-dmg = Bonus de DGT Hydro
ui-stat-anemo-dmg = Bonus de DGT Anémo
ui-stat-electro-dmg = Bonus de DGT Électro
ui-stat-dendro-dmg = Bonus de DGT Dendro
ui-stat-cryo-dmg = Bonus de DGT Cryo
ui-stat-geo-dmg = Bonus de DGT Géo

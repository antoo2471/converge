---
name: Converge
description: Un espace desktop chaleureux pour vos modèles et vos conversations.
colors:
  background: "#151210"
  sidebar: "#201a17"
  surface: "#29221e"
  raised: "#352b25"
  text: "#f6f3f0"
  secondary: "#d2c9c2"
  muted: "#a99d94"
  accent: "#eee9e4"
  accent-text: "#231c18"
typography:
  display:
    fontFamily: "Manrope Variable, Inter Variable, sans-serif"
    fontSize: "46px"
    fontWeight: 800
    lineHeight: 1.2
    letterSpacing: "-0.045em"
  body:
    fontFamily: "Manrope Variable, Inter Variable, sans-serif"
    fontSize: "14px"
    lineHeight: 1.9
  label:
    fontFamily: "Manrope Variable, Inter Variable, sans-serif"
    fontSize: "12px"
    fontWeight: 650
  code:
    fontFamily: "Cascadia Code, SFMono-Regular, Consolas, monospace"
    fontSize: "12px"
    lineHeight: 1.6
rounded:
  control: "10px"
  surface: "14px"
  window: "0px"
  settings: "20px"
spacing:
  compact: "8px"
  control: "12px"
  section: "24px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-text}"
    rounded: "{rounded.control}"
    padding: "10px 16px"
    height: "40px"
---
# Design System: Converge

## Overview

Direction explicitement corrigée par l’utilisateur à partir de ses captures d’une application musicale : une fenêtre immersive, chaude et sombre ; une navigation verticale étroite ; des contrôles intégrés et des paramètres en panneau structuré. La référence gouverne cette refonte. Les écrans restent ceux d’un outil de conversation.

La nouvelle couche normative est src/redesign.css, chargée après src/styles.css. Les composants gardent leur fonctionnement natif. Aucune conversation ni aucun fournisseur fictif ne sert de décoration.

## Colors

Bruns presque noirs, texte ivoire et accents blancs. Les lavis chauds restent limités au haut de la fenêtre ; pas de halo derrière les cartes. Le thème clair emploie un fond #f4efea et un texte #2a211c.

## Typography

Manrope Variable est auto-hébergée et porte l’interface. Titres compacts et gras ; texte de conversation à 14 px. Inter reste disponible pour les contenus antérieurs. Cascadia Code, avec SFMono-Regular, Consolas et monospace en repli, est réservée au code, aux chemins techniques et à l’éditeur d’artifacts ; elle n’est pas utilisée pour la navigation ou les titres.

## Layout

L’interface remplit la fenêtre bord à bord, sans marge ni fond noir extérieur. Barre de fenêtre de 48 px, rail de 68 px (58 px en fenêtre étroite). L’historique s’ouvre dans un panneau rétractable. Accueil aligné à gauche, avec uniquement les conversations existantes. Le composeur forme un dock inférieur ; sa largeur maximale est de 1100 px. Les messages conservent une mesure de lecture plus étroite.

Les paramètres sont une fenêtre de 1060 × 710 px maximum avec navigation de 245 px ; les contenus défilent indépendamment. Les adaptations principales interviennent à 1100 et 850 px.

## Elevation & Depth

Les surfaces ordinaires sont séparées par leurs tons et une faible opacité, plutôt que par des bordures répétées. Les ombres sont réservées aux menus et aux panneaux superposés. Le fond est flouté seulement derrière les paramètres.

## Shapes

Contrôles de 10 px, surfaces de 14 px, cadre racine sans arrondi intérieur, paramètres de 20 px. Bouton d’envoi de 32 px avec coins de 10 px. Les icônes proviennent de Lucide et gardent des dimensions cohérentes.

## Components

- Fournisseurs : liste des connexions de l’utilisateur et accès à la bibliothèque de plugins. Recherche, catégories et entrées compactes avec logo, description, capacités et action de configuration. Seuls les plugins utilisables apparaissent ; aucun compte ni serveur n’est préconfiguré. Les vrais fichiers SVG de marque apparaissent aussi dans le rail des modèles, avec une seule variante visible selon le thème.
- Sélecteur de modèles : popup sombre intégré, recherche, navigation clavier, groupes lisibles par fournisseur ; aucun select système dans le composeur.
- Composeur : texte en haut, un bouton « + » en bas à gauche, choix compact du modèle juste avant Envoyer à droite. Pièces jointes, collage et comparaison sont regroupés dans le menu « + ». Aucun bouton de génération d’images, texte d’aide permanent, pied de barre ni réglage de contexte dans le composeur. Collage Ctrl+V et dépôt restent directs ; les miniatures et les erreurs réelles restent visibles.
- Paramètres : catégories latérales, recherche et fermeture par Échap ; le contexte de travail reste visible derrière.
- Generative UI : réutilise les couleurs, les contrôles et les coins de l’application.

## Do's and Don'ts

Conserver la densité, la chaleur et la lisibilité des références utilisateur. Ne pas réintroduire un accueil marketing centré avec un slogan géant, une teinte verte généralisée, des listes de fournisseurs préremplies ni des données de démonstration. La validation visuelle reste ouverte : l’utilisateur a demandé cette refonte avant une nouvelle campagne de tests.

Les questions sont alignées à droite dans une bulle sobre ; les réponses restent ouvertes à gauche. Les actions de message sont accessibles sous leur contenu. Le nommage et les confirmations utilisent des dialogues intégrés, avec annulation par Échap et gestion du focus.

Le sélecteur de modèles partage le même navigateur dans le composeur et sous les réponses : rail vertical de fournisseurs à gauche (pastilles avec initiales), recherche et modèles du fournisseur actif à droite. Il flotte hors du conteneur défilant et s’ouvre selon la place disponible, sans déplacer les conversations.

## Generative UI

Les interfaces suivent une composition compacte inspirée de la référence fournie : un seul cadre extérieur, sections ouvertes, sans bandeau automatique ni pied technique. Les métriques sont à 22 px ; les titres de section à 13 px, les légendes et variations à 10–11 px. Contrôles courts, boutons en pilule et choix radio segmentés. Les groupes de métriques s’adaptent à la largeur du composant.

Couleurs sémantiques réservées aux données : courbes #76b3f0, positif #7cceaa, attention #e5bc75, négatif #ec9791. En clair : #276eaf, #257348, #8a5c13, #b33b36. Elles ne colorent pas les surfaces de navigation. Les badges et notices expriment un état ; les données illustratives doivent être identifiées comme telles par le modèle. Aucun faux indicateur de connexion ou de données en direct.

Converge UI est proposé spontanément quand la visualisation ou la manipulation aide : recettes ajustables, schémas, comparaisons et plans. Les salutations et réponses simples restent en texte. Le titre du document sert à l’accessibilité ; aucun bandeau titre/Interactif/réinitialisation n’est ajouté par le renderer.

## Apparence personnalisable

La section Apparence expose les palettes, cinq couleurs personnalisables, une image locale avec cadrage/position/opacité/flou, les surfaces, arrondis et bordures, trois familles typographiques, la taille et la largeur de lecture, la densité, les actions au survol et la réduction du mouvement. Les valeurs originales sont les valeurs par défaut ; les préférences utilisateur priment sur ces tokens. L’opacité concerne les panneaux internes. Les images de fond restent locales et ne sont pas jointes aux messages IA. Aperçu direct et réinitialisation sans dialogue bloquant.

Les couleurs se choisissent dans un composant intégré avec plan saturation/luminosité, curseurs et saisie HEX, sans sélecteur natif. Les dégradés du plan et de la bande de teinte codent les couleurs disponibles. Le halo supérieur possède sa couleur et son intensité. Les fonds GIF animés sont autorisés, comme les autres images locales, jusqu’à 12 Mo.

La section Réponses de l’IA remplace les contrôles techniques de génération par quatre profils : Concis, Équilibré, Approfondi et Créatif. Une liste radio avec explications courtes suffit ; pas de compteurs de tokens ou de curseur de température. Les préférences personnelles restent dans un volet facultatif.

Les outils sont présentés en quatre catégories : Fichiers et projets, Commandes, Web et Ordinateur. Des libellés d’usage remplacent les identifiants d’API. Dossier de travail et recherche affichent leur configuration manquante. Les actions individuelles sont repliées ; aucune console JSON dans les paramètres grand public. Tous les outils sont activés par défaut ; les désactivations suivantes persistent. Les confirmations et l’isolation restent assurées par le backend.

Les outils apparaissent comme des activités du modèle dans le fil, sans bouton Examiner/Exécuter. Une demande affiche la cible et les modifications utiles, puis Autoriser, Refuser et Tout autoriser dans cette conversation. L’état et la sortie se mettent à jour ; le résultat revient automatiquement au modèle. Un contrôle dans le fil permet de révoquer l’autorisation de conversation. Les demandes restent soumises au dossier choisi et ne sont jamais rejouées depuis l’historique.

Les espaces de travail sont propres à chaque conversation et accessibles depuis +. Un discret raccourci au-dessus du composeur indique le dossier par défaut et le nombre de dossiers supplémentaires. La carte outil distingue Rédaction, attente d’autorisation, écriture et résultat. Les compteurs +/− sont liés aux données réelles ; un compteur de préparation ne prétend pas modifier le disque. Les diffs ont des lignes vertes/rouges et les animations respectent la réduction de mouvement. Les chemins et contenus sont lisibles sans échappement JSON.

Les appels successifs d’un tour assistant forment un seul ensemble : nom du modèle au début, actions de message à la fin, espacement réduit entre les étapes. Les comparaisons entre modèles restent distinctes.

La catégorie Web fonctionne immédiatement avec la recherche intégrée ; un moteur personnel est un réglage facultatif. Les activités du terminal indiquent leur phase réelle. Les résultats du contrôle du bureau affichent la capture utile, sans exposer son encodage ni ses métadonnées techniques dans le fil.

L’effort de raisonnement est un contrôle discret à côté du modèle dans le composeur. Son menu personnalisé utilise les surfaces et la typographie existantes, propose Auto par défaut et se ferme au clavier ou au clic extérieur. La sélection persiste par modèle ; les niveaux disponibles dépendent du protocole et de la compatibilité du serveur.

Le panneau d’effort reprend exclusivement les surfaces, couleurs et la typographie Converge : titre Raisonnement et niveau sur une ligne, rail fin à crans, poignée de 16 px et retour Auto en texte. Aucun éclair, accent rose ou duplication du sélecteur de modèle. La piste utilise --accent et les surfaces --raised ; les préférences d’apparence et les deux thèmes restent appliqués.

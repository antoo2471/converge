# Converge

Client desktop Tauri 2 / Rust / React / TypeScript pour vos propres fournisseurs d’IA.

## Démarrer

```sh
npm ci
npm run desktop
```

Prérequis Windows : Rust, les outils de compilation C++ MSVC, Node.js et WebView2. `npm run dev` ouvre uniquement le frontend ; les connexions, le coffre système et le presse-papiers natif nécessitent l’application Tauri.

## Fournisseurs

Aucun fournisseur n’est préinstallé. Dans **Fournisseurs → Ajouter un fournisseur**, renseignez le nom, l’URL de base, le format des requêtes, la clé si nécessaire et les identifiants des modèles. La découverte utilise l’endpoint de la connexion choisie. Les chemins de conversation et de découverte, les headers et les paramètres JSON supplémentaires sont configurables.

Formats disponibles : OpenAI compatible, Anthropic compatible, Ollama compatible et ComfyUI. Ce sont des adaptateurs de requêtes ; aucune marque ni adresse de serveur n’est préconfigurée. Les connexions existantes sont conservées lors des mises à jour.

**Fournisseurs → Parcourir les fournisseurs** ouvre la bibliothèque avec recherche et catégories. Elle propose ChatGPT, Antigravity, Claude Code, Grok, Kimi, MiniMax, Z.ai / GLM, OpenCode Go / Zen, Local AI et Custom API. Aucun compte n’est connecté automatiquement.

ChatGPT utilise sa connexion OAuth propre à Converge via Codex App Server : modèles découverts, streaming, efforts, limites exposées et outils avec autorisations Converge. Les images natives renvoyées par Codex sont affichées dans le fil. Voir [le plugin ChatGPT](docs/chatgpt-provider.md) et [les nouveaux fournisseurs](docs/account-providers.md).

## Interface

Navigation par rail, historique rétractable, sélecteur de modèles avec recherche, paramètres en panneau et thèmes sombre/clair. La fenêtre utilise sa propre barre de titre avec les commandes réduire, agrandir et fermer.

Collez une image avec **Ctrl+V** dans le composeur ou déposez-la dessus. Le bouton de collage utilise aussi le presse-papiers natif Windows. Activez la capacité **Images et vision** du fournisseur pour transmettre des images à ses modèles. Les pièces jointes restent visibles et supprimables avant envoi.

## Données et sécurité

Les conversations, projets et images sont stockés localement dans SQLite. Les clés et headers sensibles restent dans le coffre du système. Les exports excluent ces identifiants. Les outils activés sont demandés et exécutés par l’IA. Les autorisations sont présentées dans le chat : autoriser une action, refuser ou tout autoriser dans cette conversation. Les autorisations globales expirent à la fermeture et restent limitées aux espaces de travail de la conversation ; elles sont révocables dans le fil. Les résultats sont renvoyés automatiquement au modèle. Le terminal Windows s’exécute dans un AppContainer ; les artifacts sont servis depuis une origine dédiée et affichés dans un iframe sandboxé.

## Construction

```sh
npm run build
npm run tauri build
```

La direction visuelle actuelle et ses tokens sont documentés dans [DESIGN.md](DESIGN.md).

Les espaces de travail se gèrent dans le menu + du composeur, séparément pour chaque conversation. Plusieurs dossiers peuvent être ajoutés et conservés. Un nouvel emplacement demandé par le modèle est proposé à l’utilisateur avant son ajout. Les écritures créent leurs dossiers parents et utilisent un fichier temporaire puis un remplacement atomique ; les compteurs affichent les lignes préparées durant la génération et le diff réel lors de l’écriture.

Le terminal choisit automatiquement son interpréteur : `cmd` pour les exécutables courants et PowerShell pour sa syntaxe. `shell: "powershell"` et `shell: "cmd"` imposent un interpréteur. En lecture seule, il accède directement à l’espace de travail autorisé, sans copier le dossier ; Windows bloque les écritures. Pour modifier des fichiers avec une commande, `readOnly: false` conserve une copie isolée et `applyChanges: true` demande ensuite l’application des changements. Cette copie reste limitée à 512 Mo / 40 000 fichiers : utiliser un sous-dossier de projet pour les commandes de modification. Les écritures par les outils fichiers ne nécessitent pas cette copie.

La recherche Web et images possède un moteur intégré Bing, sans clé ni configuration. Un endpoint SearXNG personnel reste facultatif ; une page de vérification ou une erreur déclenche un repli explicite vers Bing. Les résultats indiquent le moteur effectif, les sources et les éventuels avertissements.

Le contrôle du bureau conserve une session par conversation. `computer.windows` liste les fenêtres ; `computer.focus` renvoie une capture de la fenêtre cible. Les résultats `computer_observation` contiennent une image JPEG, un `frameId`, les dimensions de l’image et du bureau physique. Les coordonnées des actions utilisent les pixels de cette image : Converge assure la conversion, y compris sur plusieurs écrans et avec mise à l’échelle Windows. Une fenêtre déplacée ou un ancien `frameId` impose une nouvelle capture. Chaque action renvoie une capture actualisée ; seule la dernière est transmise dans le contexte visuel. La vision doit être activée sur le fournisseur.

`computer.actions` exécute une courte séquence (12 étapes maximum) de clics, double-clics, déplacements, glisser-déposer (`path: [{x,y},…]`), saisies, raccourcis, défilements et attentes. Les permissions individuelles restent appliquées. Échap interrompt la séquence ; les actions système déjà effectuées ne sont pas annulées. Le contrôle Windows agit au premier plan et reste soumis aux restrictions de Windows, notamment les applications élevées et le bureau sécurisé. Il ne suppose jamais qu’un clic envoyé a atteint son objectif : le modèle doit vérifier la nouvelle image.

Le contrôle du bureau attend 700 ms après les actions avant la capture (`settleMs` entre 0 et 5000 pour les transitions lentes). Cette attente est annulable. Le tour s’arrête après 12 échanges de contrôle ou 32 étapes, avant une quatrième demande identique ou après deux échecs successifs ; le fil explique l’arrêt. Le modèle reçoit également une consigne explicite de terminer dès que le résultat demandé est visible.

L’effort de raisonnement se choisit à côté du modèle et persiste par connexion/modèle : Auto, Faible, Moyen, Élevé, Très élevé, Maximum. Auto n’ajoute aucun champ d’effort. Les API compatibles OpenAI reçoivent `reasoning_effort`, les endpoints Anthropic `output_config.effort`, Ollama gpt-oss `think` (low/medium/high). Les niveaux acceptés restent propres au modèle ; une requête refusée affiche une erreur explicite sans simuler un changement. Pour le proxy Codex local modifié, exécuter `node scripts/patch-proxy-reasoning.mjs` après le patch des outils et redémarrer le proxy. Ce script vérifie la conversion Chat Completions/Responses hors ligne.

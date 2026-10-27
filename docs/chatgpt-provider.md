# Plugin ChatGPT

## Connexion

Dans **Fournisseurs → Parcourir les plugins**, choisissez ChatGPT, puis **Se connecter avec ChatGPT**. Terminez le consentement dans le navigateur et enregistrez la connexion. Codex CLI doit être installé ; un chemin absolu vers son exécutable reste configurable.

La connexion utilise exclusivement un profil OAuth Converge. La session Chat grand public et la réutilisation de credentials Codex externes sont retirées. Le chargement supprime leurs anciens paramètres ; une ancienne connexion partagée doit être reconnectée, sans supprimer ni déconnecter les identifiants du CLI.

L’App Server privé utilise un CODEX_HOME propre à la connexion et cli_auth_credentials_store="keyring". Le CLI réalise le callback OAuth, conserve les identifiants dans le coffre système et gère leur renouvellement. Converge ne lit ni ne copie les tokens. Les modèles et efforts sont découverts via model/list ; ce catalogue ne garantit pas les droits d’accès. Les limites exposées viennent de account/rateLimits/read.

Sous Linux, les processus Codex et navigateur n’héritent pas des entrées de `LD_LIBRARY_PATH` situées dans le `APPDIR` de Converge. Cela évite que le lanceur Node du CLI ou `xdg-open` charge les bibliothèques de l’AppImage à la place de celles du système. Les chemins personnalisés hors de ce dossier restent conservés et l’environnement de Converge lui-même n’est pas modifié.

## Conversation et outils

Converge conserve l’historique durable dans SQLite. Chaque requête utilise un thread Codex éphémère avec le contexte choisi. Branches, exclusions et changements de modèle restent maîtrisés par Converge ; aucun historique caché ne s’accumule dans un thread antérieur. La reprise après redémarrage utilise les messages locaux, sans restaurer le raisonnement interne du CLI.

Le rendu universel prend en charge Markdown, LaTeX, code, Converge UI, artifacts, images d’entrée selon le modèle et documents extraits. Les outils dynamiques rejoignent le Tool Engine et ses permissions GUI, espaces de travail, diffs et annulations. Leur résultat réel revient au même tour App Server, sans nouveau tour frontend.

Le CLI fonctionne en lecture seule dans un dossier privé. Hooks, shell, apps, plugins et sous-agents sont désactivés ; le profil neuf n’hérite pas des MCP de Codex CLI. Les demandes natives d’élévation sont refusées. Les actions locales utilisent les permissions Converge. Les tours sont bornés à 48 outils et bénéficient du garde-fou Computer Use.

## Images

Les événements item/completed de type imageGeneration sont validés puis transmis comme vraies pièces jointes. PNG, JPEG et WebP sont acceptés ; les fichiers annoncés ne sont lus que sous le dossier privé de la connexion. Les erreurs et limites d’images restent explicites. Le service décide de leur disponibilité effective pour chaque compte et modèle.

Cette capacité de chat ne transforme pas Codex en endpoint autonome Image Engine. capabilities.images reste réservé aux moteurs compatibles avec ce dernier. Les images natives sont également conservées dans l’historique local. Les images raster Markdown en data URI sont rendues ; les images distantes et SVG data URI restent bloquées.

## Architecture et bibliothèque

src-tauri/src/chatgpt sépare fournisseur, transport stdio JSON-RPC, authentification, découverte, capacités et normalisation. Le frontend utilise le même composeur et le même rendu que les autres fournisseurs. La bibliothèque tire ses entrées du Provider Registry, avec catégories, recherche, descriptions et configuration. Elle n’affiche que les plugins implémentés.

Les fichiers monochromes du logo sont conservés sans modification depuis les ressources officielles OpenAI. Une seule variante est visible selon le thème et la surface du rail des modèles.

## Vérification

Les tests couvrent permissions GUI, modèles, efforts, documents, confidentialité des erreurs, validation et affichage d’images, recherche et navigation du catalogue. Les tests réseau ignorés par défaut nécessitent CONVERGE_CODEX_TEST_HOME pointant vers un profil OAuth Converge connecté ; ils consomment des générations :

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib chatgpt::tests::live_codex_catalog_and_stream -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib chatgpt::tests::live_codex_image_generation -- --ignored --nocapture
```

Le consentement OAuth initial est réalisé par l’utilisateur. Le test réel de génération d’image a été exécuté avec un profil Converge connecté et a reçu une vraie image BMW M3.

Sources : [App Server](https://learn.chatgpt.com/docs/app-server), [authentification Codex](https://learn.chatgpt.com/docs/auth), [configuration Codex](https://learn.chatgpt.com/docs/config-file/config-basic), [ressources de marque](https://openai.com/brand/).

# Converge 26.10.1 — état final

Implémenté dans le dépôt :
- Tutoriel GitHub en trois étapes, callback exact et copie dès la préparation ; suppression de l’étape Preview Google.
- Bibliothèque skills.sh utilisée par npx skills, recherche réelle et import du seul skill sélectionné ; imports personnalisés conservés.
- Restauration des autorisations OAuth depuis le coffre système ; déconnexion explicite persistante.
- Sessions NDJSON persistantes Antigravity et Claude Code, contexte incrémental, détection des éditions/branches, expiration après deux minutes d’inactivité. Les outils natifs des clients restent désactivés.
- Instructions et retours d’outils : résultats internes à synthétiser pour l’utilisateur ; appels hors du raisonnement.
- PATH installé, Node/npm opérationnels dans l’AppContainer, sorties PowerShell lisibles ; PowerShell 7 et cache du runtime installé via Microsoft Store. Aucune exécution de commande utilisateur hors sandbox.
- Adaptateurs natifs macOS/Linux : capture et entrée xcap/enigo, terminal Seatbelt/bubblewrap. Contrôle Linux limité explicitement à X11.
- Indicateur Converge a le contrôle, Échap et arrêt natif des actions et de l’inférence, arrêt des boucles d’observations, délai après actions.
- Workflows six architectures, numéro mensuel automatique, publication après réussite des builds, updater signé et page de mise à jour.
- README illustré sans emoji, diagramme, badges et guides extensions/releases.

Vérifications exécutées :
- 107 tests frontend réussis ; build de production Vite réussi.
- 73 tests Rust réussis ; 12 tests optionnels ignorés dans la suite standard.
- Test interactif Windows dans une fenêtre jetable : focus, capture, clic, saisie et rejet d’une ancienne capture réussis.
- Mesure réelle des commandes : cmd 227–261 ms ; PowerShell 2490–3014 ms. Les commandes natives utilisent cmd automatiquement.
- 2 tests de versionnement réussis, y compris synchronisation des cinq manifestes ; YAML des workflows valide.
- Build natif Windows réussi ; packaging NSIS x64 de validation (debug) généré et version de fichier vérifiée.
- Installateur : src-tauri/target/debug/bundle/nsis/Converge_26.10.1_x64-setup.exe
- SHA256 : 9B4085F8BF170D2DAA4B6245837C7FA898198468FB8C940CC345904A02F5EC1E
- Revue visuelle des composants réels dans Chromium : corrections GitHub/mises à jour résolues, verdict ship sur ces corrections. DESIGN.md préservé.

Limites de validation :
- Les builds macOS, Linux, ARM64 et le packaging de production doivent encore être exécutés par GitHub Actions.
- La connexion finale de chaque compte, le raccourci global du bandeau dans une session utilisateur et l’installation d’une release updater publiée ne sont pas couverts par les tests locaux. Le contrôle natif clic/capture/saisie a été testé.
- Le moteur Cua et le skill Computer Use n’ont pas pu initialiser leur kernel ; les captures de revue ont utilisé les vrais composants React en Chromium headless, sans connexion ni données simulées.

Publication :
- Aucun commit, push ou publication effectué.
- Configurer TAURI_SIGNING_PRIVATE_KEY depuis ~/.converge-release/updater.key ; ce fichier privé reste hors du dépôt et son contenu n’a jamais été affiché.
- Pousser les changements puis lancer Publish release. Le secret est vérifié avant allocation du numéro. Procédure : docs/releases.md.
- Le disque plein a été débloqué par cargo clean sur les seuls fichiers générés du projet, puis l’utilisateur a confirmé avoir libéré de l’espace. Les tests utilisent désormais un fichier sparse.
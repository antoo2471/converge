# Extensions : apps MCP et skills

L’onglet **Plugins**, en haut de la navigation, regroupe les apps à découvrir, les connexions personnelles, la recherche dans le registre MCP et les skills. Les fournisseurs de modèles restent indépendants.

## Connecter une app

Dans Découvrir, le bouton + ouvre une fenêtre OAuth. Converge lance le navigateur, reçoit le retour sur une adresse locale et établit une vraie connexion MCP. L’état connecté et les outils affichés proviennent du serveur. Aucun compte n’est connecté lors de l’installation.

Certains services exigent un client OAuth enregistré par l’utilisateur ou une approbation de l’éditeur. La configuration avancée accepte un client ID et son secret. Les apps sans endpoint public vérifié proposent les connecteurs HTTP du registre MCP ou une URL personnalisée ; la présence d’une app au catalogue ne garantit pas l’existence d’un connecteur officiel. Le nom du serveur et son éditeur doivent être vérifiés avant autorisation.

### Google officiel

Gmail, Drive, Docs, Sheets, Slides, Calendar, Chat et Contacts utilisent les endpoints officiels Google, séparément. L’accès dépend d’un projet Google Cloud, des API activées et d’un client OAuth configuré pour l’agent. Le callback affiché dans Converge doit être enregistré, avec le port 32145. La demande inclut l’accès hors ligne pour renouveler la connexion.

Référence : https://developers.google.com/workspace/guides/configure-mcp-servers

Les autres apps Google restent séparées au catalogue ; elles ne sont pas présentées comme disposant toutes d’un serveur officiel public.

## MCP personnalisé

Ajouter → serveur MCP permet de configurer Streamable HTTP (OAuth, token ou sans authentification), ou un processus local stdio avec exécutable, arguments, variables d’environnement et dossier de travail. Un serveur stdio s’exécute **avec les droits de l’utilisateur**, sans sandbox système implicite : utilisez uniquement des exécutables de confiance. Le catalogue n’exécute aucun paquet pendant sa consultation.

Les tokens, secrets OAuth, en-têtes et variables sensibles sont conservés dans le coffre système. Les modifications partielles conservent les secrets non remplacés. Changer l’URL ou le client OAuth invalide les anciens identifiants de cette connexion. Déconnecter arrête la session ; retirer la connexion supprime ses identifiants locaux.

## Skills

Créer un skill permet d’écrire un SKILL.md. L’import accepte un fichier SKILL.md, un dossier ou une archive ZIP contenant un skill. Le front matter YAML doit fournir name et description. Converge copie les fichiers dans son stockage privé ; supprimer le skill ne supprime pas l’original. Les traversées de chemins et liens sortant du dossier sont refusés. Aucun script du skill ne s’exécute automatiquement.

L’onglet **Bibliothèque** recherche dans la même API skills.sh que `npx skills`. Installer télécharge la branche par défaut du dépôt GitHub annoncé et importe uniquement le skill sélectionné avec ses ressources. L’import refuse les chemins extérieurs, les liens symboliques et les archives dépassant les limites. Les instructions d’un skill restent du contenu tiers : leur présence ne donne aucun droit supplémentaire aux outils.

### GitHub

Le parcours intégré explique la création d’une OAuth App personnelle, fournit la Homepage URL et l’adresse exacte de retour dès la première étape, puis recueille Client ID et secret. La troisième étape ouvre GitHub. Aucun Personal Access Token n’est requis par ce parcours. Les autorisations demandées et les règles d’organisation restent visibles avant connexion.

### Retrouver ses connexions

Les autorisations OAuth et leurs renouvellements sont conservés dans le coffre système. Au démarrage, Converge rétablit les connexions OAuth activées possédant déjà une autorisation, sans ouvrir le navigateur. Déconnecter désactive cette reconnexion ; reconnecter la réactive. Un compte révoqué ou une nouvelle exigence du service peut nécessiter une nouvelle autorisation.

## Interface avec les modèles

Le contexte système contient les descriptions des skills activés et les connexions MCP disponibles. Le modèle charge les instructions via `skills.read {skillId,path}` puis découvre les outils via `mcp.list {serverId}`. Les autres opérations sont :

- `mcp.call {serverId,toolName,arguments}` : outil réellement annoncé par le serveur.
- `mcp.read {serverId,uri}` : ressource MCP.
- `mcp.prompt {serverId,name,arguments}` : prompt MCP.

Ces opérations passent par les autorisations Converge de la conversation. Les contenus des serveurs et des skills sont des données externes ; ils ne confèrent aucun privilège supplémentaire.

L’implémentation est dans `src-tauri/src/extensions`, fondée sur le SDK Rust rmcp. Les commandes IPC de gestion sont réservées à la fenêtre principale. Les configurations sont dans la table SQLite `extensions` ; les secrets restent hors des exports de conversations. Le catalogue visuel est dans `src/core/appCatalogue.ts`, et les composants dans `ExtensionsPage.tsx` et `AppConnectDialog.tsx`.

## Vérification

Les tests couvrent un vrai échange MCP HTTP et stdio, appels d’outils, ressources, prompts, arrêt de session, secrets partiels, annulation OAuth et confinement des imports. Un test réseau opt-in vérifie les métadonnées Google et la construction du lien PKCE sans autoriser de compte :

```powershell
$env:CONVERGE_MCP_METADATA_TEST='1'
cargo test --manifest-path src-tauri/Cargo.toml --lib extensions -- --include-ignored
```

L’autorisation finale d’un compte et les restrictions propres à chaque éditeur nécessitent une vérification avec ce compte ; elles ne sont pas simulées par ces tests.

## Configuration Google guidée

Le parcours comprend trois pages : préparation du projet, des API et du consentement ; configuration du client Web avec l’option agent, le callback exact, le Client ID et le secret ; récapitulatif puis connexion dans le navigateur. Aucun appel OAuth n’a lieu pendant la lecture du tutoriel. Retour conserve les champs, Annuler termine une connexion en cours et le succès nécessite une véritable connexion MCP. Les connecteurs communautaires gardent leur propre parcours.

### Compatibilité du protocole Google

Les endpoints MCP officiels Google utilisent le cycle `initialize` du protocole 2025-03-26. Converge sélectionne cette version pour ces endpoints ; le défaut récent du SDK (2026-07-28, cycle `server/discover`) n’est pas appliqué à Google. Un test de transport protège ce choix et le contrôle de domaine reste strict.

Un échec d’initialisation après OAuth affiche une erreur distincte du login. Pour les endpoints Google officiels seulement, un diagnostic limité à 8 secondes répète un handshake sans appeler d’outil et extrait le statut HTTP et certains codes structurés. Les corps d’erreur bruts, tokens et secrets ne sont pas affichés.

# Connexions de fournisseurs

La bibliothèque décrit les intégrations ; elle ne crée aucun compte ni abonnement. Les connexions ajoutées persistent dans SQLite. Les clés API restent dans le coffre système.

| Plugin | Authentification | Dépendance |
| --- | --- | --- |
| Claude Code | OAuth du compte Claude | `claude` officiel |
| Grok | OAuth Grok Build | `grok` officiel |
| Kimi | OAuth Kimi Code | `kimi` officiel |
| MiniMax | OAuth MiniMax global | `mmx` officiel, paquet npm `mmx-cli`, Node |
| Antigravity | OAuth Google du client Antigravity | `agy` officiel |
| Z.ai / GLM | OAuth ZCode du Coding Plan personnel, ou clé API | Navigateur et API Coding Plan dédiée |
| OpenCode Go / Zen | Clé API | Service Go ou Zen explicitement choisi |

Les clients OAuth s’ouvrent dans une fenêtre native pour terminer leur authentification officielle (navigateur, code d’appareil ou choix de compte selon le client). Converge ne lit, n’affiche et ne copie pas leurs tokens. Un client absent affiche son état et un lien vers son installation. Le chemin d’un exécutable peut être configuré ; il est supprimé lors de l’import d’un espace partagé.

Z.ai propose une connexion de compte : Converge initialise le flux ZCode, ouvre la page Z.ai dans le navigateur et observe sa validation avec un secret de polling propre à cette tentative. Le retour utilise directement le callback serveur Z.ai `/api/v1/oauth/cli/callback/zai`, qui finalise le flux observé par Converge. La page intermédiaire réservée aux liens `zcode://` n’est pas utilisée. Les jetons de connexion restent dans le backend, en mémoire ; une clé nommée `converge-<identifiant de connexion>` est créée ou réutilisée dans le projet personnel, puis conservée dans le coffre système. L’abonnement personnel actif est vérifié avant l’enregistrement. La clé manuelle reste une option pour les plans d’équipe. Aucun quota Start Plan réservé à ZCode n’est utilisé, et aucun prompt ni User-Agent ZCode n’est usurpé. L’URL Coding Plan n’est jamais remplacée automatiquement par une URL facturée à l’usage. Le protocole de connexion est adapté de l’[implémentation publique ZCode](https://github.com/magpie-community/plugins/tree/main/packages/zcode) ; son initialisation a été vérifiée contre le service réel, la finalisation exige l’autorisation de l’utilisateur.

OpenCode : Zen est le choix initial pour une clé standard ; Go nécessite un abonnement Go actif. Un catalogue de modèles ne garantit pas les droits d’accès. Les réponses du service sont interprétées sans révéler leur contenu brut : abonnement manquant, clé refusée, modèle gratuit réservé à l’application officielle, restrictions régionales et quota. Aucun basculement de service ou de modèle payant n’a lieu automatiquement.

## Transports et outils

`cli_providers.rs` exécute les clients sans shell, alimente leur entrée standard et convertit leurs réponses vers les événements universels de Converge. Les wrappers npm de mmx sont résolus vers leur entrée JavaScript et exécutés directement par Node. Les requêtes et réponses sont bornées, les erreurs ne divulguent pas stderr, et l’annulation tue le processus possédé par la requête.

Chaque connexion a un profil distinct : `CLAUDE_CONFIG_DIR`, `GROK_HOME`, `KIMI_SHARE_DIR`, `MMX_CONFIG_DIR`, ou un home de processus dédié pour Antigravity. La gestion des identifiants de ces profils reste celle du client officiel. Les clés API héritées du processus parent sont retirées. La suppression d’une connexion retire son profil local.

Claude et Grok démarrent sans leurs outils intégrés. Kimi utilise un agent sans outils ni sous-agents. MiniMax utilise uniquement sa commande de conversation. Antigravity reçoit une politique native `permissions.deny: ["*"]` dans son profil dédié ; ses outils peuvent être annoncés dans `init`, mais leur exécution est refusée par le client. Les appels Converge utilisent le protocole structuré du chat et passent par le gestionnaire de permissions existant.

Grok traite `--tools ""` comme la liste par défaut, pas comme « aucun outil ». Converge utilise une liste autorisée non vide puis retire cette liste et les helpers MCP via `--disallowed-tools`, et vérifie que l’événement `system/init` annonce zéro outil. Son home de processus est isolé et les compatibilités Claude/Cursor/Codex sont désactivées via `GROK_CONFIG` afin de ne pas charger les hooks personnels du PC. Un refus HTTP 426 est présenté comme une mise à jour requise ; le payload brut n’est pas exposé. Le client doit respecter la version minimale imposée par xAI ; le flux Converge avec Grok 4.7 a été testé avec Grok CLI 1.0.70.

Le contexte est reconstruit depuis la conversation Converge pour chaque requête, sans reprendre un historique personnel du CLI. Les clients d’agents reçoivent un transcript explicite ; mmx reçoit un tableau de messages. Le transport Claude accepte aussi les pièces jointes image. Les autres transports de compte restent déclarés sans vision : aucune image n’est silencieusement ignorée.

`default` signifie « modèle configuré par le compte », pas un modèle découvert ni une garantie d’accès. Claude expose ses alias sonnet/opus/haiku. Grok et Antigravity interrogent aussi le catalogue du client. Les identifiants restent éditables et le service vérifie leur accès pendant la génération.

OpenCode adapte le transport aux familles documentées : Chat Completions, Messages, Responses et Gemini. Go et Zen conservent leurs URL distinctes ; les requêtes portent le User-Agent Converge et l’identifiant stable de conversation. Le catalogue distant reste la source des modèles disponibles. Les évolutions de routage du service doivent être reportées dans `opencode_route`.

## Vérification

Les tests couvrent le catalogue, la configuration GUI, l’absence de démarrage OAuth implicite, les clés séparées des données de fournisseur, les imports, les parseurs SSE et CLI, les messages partiels sans duplication, les caractères UTF-8 fragmentés et l’annulation d’un vrai sous-processus local de test. Les sessions OAuth et les générations authentifiées exigent une connexion utilisateur et ne sont pas simulées par ces tests.

Sources : [Claude CLI](https://code.claude.com/docs/en/cli-reference), [Grok Build](https://github.com/xai-org/grok-build), [MiniMax CLI](https://github.com/MiniMax-AI/cli), [Kimi CLI](https://moonshotai.github.io/kimi-cli/en/), [Antigravity CLI](https://www.antigravity.google/docs/cli/headless/), [permissions Antigravity](https://www.antigravity.google/docs/permissions/), [ZCode](https://zcode.z.ai/en/docs/configuration), [OpenCode Go](https://dev.opencode.ai/v2/docs/console/go/).

## DeepSeek

Deux modes indépendants : compte (par défaut) et clé API. La connexion de compte utilise le protocole PKCE publié dans [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness/tree/master/packages/credentials/deepseek-account-platform). Le navigateur ouvre uniquement `https://platform.deepseek.com/dsh/authorize`. Le backend reçoit le code sur un port loopback éphémère, vérifie l’état, l’hôte, le chemin et l’unicité des paramètres, puis échange le code avec le vérificateur PKCE. La tentative expire au plus tard après dix minutes et son annulation retire l’accès local puis sollicite `auth_cancel`. Le jeton opaque reste dans le coffre système sous `deepseek-oauth/<providerUUID>` ; aucune session du navigateur ou de Harness n’est lue.

Les requêtes de compte utilisent le format Messages (`https://api.deepseek.com/anthropic/v1/messages`) avec `x-dsh-auth-token` et la découverte sur `/models`. L’URL, les chemins, le corps additionnel et les headers ne sont pas personnalisables dans ce mode afin de cloisonner l’identifiant. Le mode clé API utilise Chat Completions et un secret séparé, sans repli automatique entre les deux authentifications. Les modèles sont chargés depuis le service, pas inventés. Les quotas et droits relèvent du compte plateforme : ce flux ne promet pas l’accès gratuit du chat grand public.

L’initialisation et l’annulation réelles du protocole ont été vérifiées. La validation de bout en bout et l’inférence nécessitent que l’utilisateur autorise son compte. Les tests locaux couvrent le callback HTTP réel, les requêtes d’authentification, le cloisonnement des headers, l’annulation périmée, les erreurs et les modes de configuration. Le logo provient de Lobe Icons (MIT), comme les autres fournisseurs.

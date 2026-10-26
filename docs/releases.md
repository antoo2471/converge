# Publier Converge

## Une seule action pour les six distributions

Le workflow **Publish release** se lance manuellement depuis GitHub Actions, sur `main`. Il calcule le prochain numéro `année.mois.release`, avec le calendrier Europe/Paris et les tags existants, met à jour les cinq manifestes, committe cette version, crée le tag et une release brouillon. La release devient publique uniquement lorsque les six builds natifs réussissent.

| Runner | Architecture | Livrables |
|---|---|---|
| Windows | x86_64 | installateur NSIS .exe et signature updater |
| Windows ARM | aarch64 | installateur NSIS .exe et signature updater |
| macOS Apple Silicon | aarch64 | .dmg et archive updater signée |
| macOS Intel | x86_64 | .dmg et archive updater signée |
| Ubuntu | x86_64 | .deb, .AppImage et signature updater |
| Ubuntu ARM | aarch64 | .deb, .AppImage et signature updater |

Un échec de build conserve la release en brouillon. Sans secret de signature, le workflow s’arrête avant d’allouer une version. Les builds de pull request produisent des artifacts CI sans utiliser la clé privée. Le dépôt doit autoriser GitHub Actions à écrire sur la branche de release et à créer des tags/releases. Une protection de branche peut demander un compte de publication dédié.

## Signature des mises à jour

La clé publique est dans `src-tauri/tauri.conf.json`. La clé privée n’est pas dans le dépôt. Sur la machine de préparation, elle est protégée dans `~/.converge-release/updater.key`. Sauvegardez-la hors du dépôt : les installations existantes font confiance à cette identité.

Dans **Settings → Secrets and variables → Actions**, ajoutez :

- `TAURI_SIGNING_PRIVATE_KEY` : contenu du fichier privé, jamais sa clé publique.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` : mot de passe de la clé si elle en possède un. La clé générée pour cette préparation n’en possède pas ; laissez ce secret absent ou vide.

Vous pouvez coller le fichier dans le formulaire GitHub, ou utiliser GitHub CLI sans imprimer son contenu :

```powershell
Get-Content -Raw -LiteralPath "$env:USERPROFILE/.converge-release/updater.key" | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo antoo2471/converge
```

Ne lancez pas `create-updater-key.mjs` pour remplacer une clé déjà distribuée. Ce script refuse une rotation involontaire lorsque la clé publique existe mais que sa clé privée n’est pas disponible.

La signature Tauri protège les mises à jour ; elle ne remplace pas une signature de code Windows ou une signature Developer ID et la notarisation Apple. Les builds macOS sont signés ad hoc. Pour une diffusion sans avertissements du système, configurez vos identités de signature Windows/Apple dans votre environnement de publication.

## Version automatique

La première version d’octobre 2026 est `26.10.1`. Les suivantes sont `26.10.2`, etc. Le compteur repart à 1 au mois suivant. Les tags sont la source de vérité. Une release échouée consomme son tag ; le prochain lancement alloue un nouveau numéro.

```sh
node scripts/release-version.mjs
node --test scripts/release-version.node-test.mjs
```

Le premier affiche le prochain numéro sans rien écrire. `--write` synchronise package.json, package-lock.json, Cargo.toml, Cargo.lock et tauri.conf.json. L’interface lit la version de package.json.

## Canal de mise à jour

Le client consulte `https://github.com/antoo2471/converge/releases/latest/download/latest.json`. Le workflow tauri-action agrège les plateformes et leurs signatures dans ce manifeste. Le client vérifie la disponibilité en arrière-plan, présente la version et installe après l’action « Installer et relancer ». Il bloque l’installation pendant une génération ou un outil actif.

Le dépôt doit être accessible sans authentification pour que les clients installés téléchargent les updates. Si vous rendez le dépôt privé, remplacez ce canal par un endpoint de distribution public ; n’embarquez aucun token GitHub dans le client.

## Vérifications de distribution

Les runners Ubuntu n’ont pas besoin de coffre de bureau pour les tests de transport : ces tests injectent une clé de fixture et communiquent avec un vrai serveur HTTP local. Les accès au coffre restent obligatoires dans le chemin de production, et un test vérifie qu’une erreur de coffre bloque tout envoi.

Avant les tests Linux, le workflow vérifie réellement la création des namespaces bubblewrap. Si AppArmor Ubuntu 24.04 la bloque, il charge un profil limité à `/usr/bin/bwrap` sur le runner éphémère. La restriction globale AppArmor et l’isolation réseau sont conservées. Un échec de ce contrôle arrête le job avant les tests. Les échanges HTTP de test et l’étape Rust ont aussi des délais limites.

Les tests frontend, les tests Rust et le packaging sont des étapes bloquantes. Les tests qui contrôlent réellement le bureau restent opt-in et utilisent une fenêtre jetable. Une machine Windows ne valide pas l’exécution native de macOS ou de Linux : consultez les six jobs avant de publier. Les signatures, la présence du manifeste updater et un essai d’installation sur chaque OS restent nécessaires pour valider une distribution finale.

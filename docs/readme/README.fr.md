<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - outil de traitement d'images ultra-rapide" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim est un outil performant de traitement d'images par lot. Il permet de redimensionner, compresser, faire pivoter et convertir des images en masse.

![PicTrim screenshot](assets/pictrim-screenshot-fr.png)

## Fonctionnalités

- Mélangez fichiers image et dossiers en entrée, ou glissez-les directement dans l'application.
- Redimensionnez par côté le plus long, boîte englobante, recadrage fixe, largeur ou hauteur.
- Exportez en JPG, PNG ou WebP, ou conservez le format d'origine.
- Configurez la qualité, la rotation, la concurrence, l'agrandissement et la gestion des fichiers existants.
- Conservez la structure des dossiers avec progression, statistiques, journaux et échecs en temps réel.

## Performances

- PicTrim vise à être un outil de traitement d'images par lot extrêmement rapide.
- Le cœur utilise Rust et libvips pour un débit élevé et une faible consommation mémoire.
- Une file de tâches en streaming et un traitement multithread rendent les très grands ensembles d'images pratiques.
- Les fichiers de sortie existants peuvent être ignorés, ce qui facilite les exécutions répétées ou incrémentales.

## Utilisation

1. Ajoutez des fichiers image ou des dossiers.
2. Choisissez un dossier de sortie.
3. Réglez la taille, le format, la qualité et les options de lot.
4. Cliquez sur « Start ».

## Plateformes prises en charge

- macOS
- Windows 10 / Windows 11 64-bit

Les builds actuels ciblent macOS et Windows x64. Les paquets Linux ne sont pas encore fournis.

## Développement

```bash
npm install
npm run dev
```

Vérifier le build frontend :

```bash
npm run build
```

Vérifier la partie Rust :

```bash
cd src-tauri
cargo check
```

La compilation ou l'édition de liens du binaire Rust nécessite libvips installé localement.

macOS :

```bash
brew install vips
```

Sous Windows, utilisez le paquet précompilé officiel de libvips et définissez `VIPS_DIR` vers le dossier extrait :

```powershell
$env:VIPS_DIR = "C:\path\to\vips-dev-8.x"
$env:Path = "$env:VIPS_DIR\bin;$env:Path"
```

Vous pouvez aussi consulter le [guide d'installation officiel de libvips](https://www.libvips.org/install.html).

## Build

```bash
npm install
npm run tauri:build
npm run release:package
```

Une fois le build terminé, `release/PicTrim/` contient la version portable. Les builds Windows créent aussi un installateur NSIS et le copient dans `release/`. `npm run release:package` génère le zip portable et `SHA256SUMS.txt` pour GitHub Releases.

## Notes de version

Les GitHub Releases actuelles fournissent des binaires non signés. macOS ou Windows peuvent afficher un avertissement de sécurité au premier lancement. Téléchargez depuis la page Releases du projet et vérifiez l'intégrité avec le fichier `SHA256SUMS.txt` inclus.

Consultez [../release.md](../release.md) pour le processus de publication détaillé.

## Licence

PicTrim est publié sous la [MIT License](../../LICENSE).

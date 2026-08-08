<p>
  <img src="assets/pictrim-logo.png" alt="PicTrim - outil de traitement d'images ultra-rapide" width="360">
</p>

[English](../../README.md) | [中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md)

PicTrim est un outil libre, gratuit et performant de traitement d’images par lot. Il permet de redimensionner, compresser, faire pivoter et convertir des images en masse. En plus des images classiques, il peut alléger les fichiers PDF, avec des résultats particulièrement visibles pour ceux qui contiennent beaucoup d’images.

## Capture d’écran

![PicTrim screenshot](assets/pictrim-screenshot-fr.png)

## Fonctionnalités

- Le traitement en continu permet de gérer facilement des dizaines de millions de fichiers.
- Redimensionnez par côté le plus long, boîte englobante, recadrage fixe, largeur ou hauteur.
- Exportez au format JPG, PNG ou WebP, ou conservez le format d’origine.
- Configurez les options courantes : qualité, rotation, nombre de tâches simultanées, agrandissement et omission des fichiers existants.
- Prenez en charge les PDF en conservant leur structure d’origine pour compresser les images qu’ils contiennent, ou en exportant ces images dans un format graphique.

## Traitement des fichiers PDF

- **Entrée** : ajoutez directement des fichiers PDF ou des dossiers qui en contiennent. PicTrim traite les images intégrées au contenu des pages PDF.
- **Conserver le format d’origine** : la sortie reste un PDF. Seules les images sont remplacées ; tout le reste du contenu reste inchangé.
- **Exporter en JPG / PNG / WebP** : les images intégrées au PDF sont exportées dans des fichiers distincts, sous des chemins tels que `<nom PDF>/page-0001-image-0001.jpg`.

## Performances

- Le cœur utilise Rust et libvips pour un débit élevé et une faible consommation mémoire.
- Une file de tâches en continu et un traitement multithread permettent de gérer facilement des dizaines de millions d’images.
- Les fichiers de sortie existants peuvent être ignorés, ce qui facilite les exécutions répétées ou incrémentales.

## Utilisation

1. Ajoutez des fichiers image, des fichiers PDF ou des dossiers.
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

La compilation ou l’édition de liens du binaire Rust nécessite l’installation locale de libvips. QPDF 12.3.2, zlib 1.3.1 et IJG libjpeg 9f sont figés et compilés statiquement ; il n’est donc pas nécessaire d’installer séparément QPDF, CMake ou libclang.

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
```

Une fois le build terminé, `release/PicTrim/` contient la version portable locale. Les GitHub Releases publient uniquement les DMG macOS signés et notariés pour Apple Silicon et Intel, ainsi que l’installateur Windows x64.

## Notes de version

Les builds macOS sont signés avec un certificat Developer ID et notariés par Apple. Les builds Windows ne sont actuellement pas signés et peuvent afficher un avertissement SmartScreen au premier lancement.

Consultez [../release.md](../release.md) pour le processus de publication détaillé.

## Licence

PicTrim est publié sous la [MIT License](../../LICENSE). Les mentions relatives aux dépendances intégrées figurent dans [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md).

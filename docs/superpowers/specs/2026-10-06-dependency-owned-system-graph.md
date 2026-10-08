# Graphe système à dépendances explicites

## Objectif

Ce refactoring retire les accès globaux aux gestionnaires système ordinaires.
Le `main.rs` propre à chaque cible compose le système, crée les gestionnaires
dans un ordre explicite, puis distribue aux exécutables uniquement les
capacités dont ils ont besoin.

## Composition du système

Les points d'entrée natifs et wasm construisent le graphe dans cet ordre :

1. task, users et time;
2. graphics si la cible dispose d'un écran;
3. le VFS, qui reçoit task/users/time;
4. network si un réseau est activé, avec task/VFS/time et la source d'aléa;
5. les périphériques et exécutables qui utilisent ces services.

`VirtualFileSystem::new` conserve task/users/time au lieu de les laisser
implicites. `network::Manager::new` conserve task/VFS/time. Les points de
composition allouent les gestionnaires destinés à vivre pendant tout le
processus avec `Box::leak` et transmettent des références `&'static`. Les
contextes ne possèdent que ces références; aucun `Arc` n'est nécessaire
uniquement pour étendre leur durée de vie.

## Applications et exécutables

`ExecutableContext` fournit task, users, VFS, time, et graphics/network
optionnels. `Standard` transporte ce contexte avec les flux d'entrée/sortie.
Le macro de montage d'exécutables associe ce contexte au point d'entrée; les
applications, leur shell, leurs onglets et leurs callbacks le transmettent ou
le conservent.

Les fonctions d'authentification reçoivent un `authentication::Context` qui
regroupe le VFS, task manager, users manager et la tâche appelante. Les
commandes des shells utilisent un `CommandContext` contenant le contexte
d'exécution, plutôt que de rechercher des gestionnaires.

Les handles de fichier et de répertoire retiennent le VFS d'origine. Leur
destructeur peut fermer proprement le handle même si plusieurs graphes système
coexistent.

## Interfaces imposées par le runtime

Les appels ABI C ne peuvent pas recevoir de références Rust. Ils utilisent un
`RuntimeContext` installé explicitement par `main`, avec les seules capacités
VFS/temps requises par cette frontière. Le logger, l'allocateur global, la
classe LVGL et le callback de tick restent process-wide lorsque les contraintes
de runtime/FFI le nécessitent.

Les opérations réseau asynchrones et leurs tâches de fond reçoivent des
références explicites aux gestionnaires. Les devices HTTP/HTTPS sont construits
avec network/task. Les callbacks LVGL utilisent l'accès FFI au gestionnaire
graphique installé par la composition, tandis que les objets `OwnedWindow`
gardent le handle de synchronisation nécessaire à leur destruction.

## Tests

Les constructeurs de managers sont publics afin que les tests puissent bâtir
des graphes indépendants. Le module `testing` compose un graphe isolé et fournit
un `Standard` muni de son contexte.

Les wrappers générés pour les tests partagent un task manager de test sérialisé,
afin que le graphe créé par `testing::initialize` utilise le même gestionnaire
que la tâche du test, quel que soit le chemin choisi pour référencer le crate.
Chaque application démarrée par un `main.rs` crée en revanche son propre task
manager explicite.

## Vérifications effectuées

- `cargo check -p native_example`
- `cargo check -p wasm_example`
- `cargo test -p task`
- `cargo test -p users`
- `cargo test -p virtual_file_system`
- `cargo test -p network`
- Compilation via `--no-run` des tests d'intégration settings, file manager,
  graphical shell, command-line shell, terminal, wasm, weather et calculator

Les tests interactifs des exécutables restent marqués `ignore` comme auparavant.
Le check complet de l'hôte wasm (`cargo check -p wasm --features default_host`)
reste bloqué par des erreurs `never_type_fallback` dans les bindings LVGL
générés sous Rust 2024; `cargo check -p wasm_example` passe.

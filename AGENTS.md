# Objectifs du projet

## Produit

Le produit doit permettre à plusieurs utilisateurs, depuis des navigateurs web distincts, de visualiser et de partager un même plateau de jeu et d'y déplacer des cartes.

Le plateau est partagé entre les participants : chaque utilisateur doit pouvoir rejoindre le plateau, voir son état courant et observer les changements effectués par les autres participants.

Les déplacements de cartes réalisés dans un navigateur doivent être propagés aux autres navigateurs afin que tous les participants conservent une vision cohérente du plateau.

Les utilisateurs doivent pouvoir interagir avec ce plateau partagé, notamment pour y déplacer des cartes.

## Lisibilité et qualité du code

La priorité du projet est la lisibilité du code. Les contributions doivent appliquer les bonnes pratiques de génie logiciel suivantes :

- privilégier des noms explicites pour les modules, types, fonctions, variables et constantes ;
- écrire des fonctions courtes, cohérentes et centrées sur une seule responsabilité ;
- structurer le code en modules clairs avec des frontières de responsabilité simples à comprendre ;
- éviter la duplication en factorisant les comportements communs sans introduire d'abstractions inutiles ;
- documenter les choix non évidents, les invariants métier et les interfaces publiques ;
- maintenir une gestion d'erreurs explicite et compréhensible ;
- ajouter ou mettre à jour les tests lorsque le comportement applicatif change ;
- préférer la simplicité et la maintenabilité aux optimisations prématurées.

## Serveur

Le serveur doit être développé en Rust.

Les choix techniques côté serveur doivent favoriser :

- la sûreté mémoire et la robustesse ;
- une organisation claire des couches applicatives ;
- des interfaces explicites entre la logique métier, l'accès aux données et l'exposition réseau ;
- la synchronisation fiable de l'état partagé du plateau et des positions de cartes entre plusieurs navigateurs ;
- une configuration reproductible et documentée.

## Clients

Les clients doivent être fournis dans une application web.

L'application web doit privilégier :

- une expérience utilisateur claire et accessible ;
- une séparation nette entre l'interface utilisateur, la gestion d'état et les appels au serveur ;
- des composants réutilisables et faciles à tester ;
- une visualisation claire du plateau partagé, des cartes et de leur position ;
- des interactions fluides pour déplacer les cartes sur le plateau ;
- une mise à jour visible du plateau lorsque les autres utilisateurs déplacent des cartes ;
- une intégration explicite avec les API exposées par le serveur Rust.

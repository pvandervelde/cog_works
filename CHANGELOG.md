# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.0] - 2026-03-07

### Doc

- Fix whitespace ([9901f70](9901f70a0c9fa4a5c5310042b7cd3f05f27dd70c))

### Spec

- Align with domain services Extension API specification ([ed0ac44](ed0ac448809cb6849fc7f87cec4dbc922d40f51f))
- Address PR review feedback on domain services alignment ([579b7e2](579b7e272d3914648af3e6eba0358f2315a1c4ee))
- Align with domain services Extension API specification (#3) ([fd8ef41](fd8ef41a0d094af75b757a908c7c1f434df5a9fa))
- Add Context Pack system and Constitutional Security Layer ([30b25db](30b25db1afe5fd32b9a6bdce110bf158fe66fa2e))
- Address PR #4 review feedback ([f5de9ba](f5de9ba36ad5ff88c86d45e751ee6f6680a75d2b))
- Address final PR #4 review feedback ([89218ad](89218ad3cd27d1cb92fd280d521052bd437e1be4))
- Evolve pipeline from fixed linear to configurable graph (PR #1) ([3073b9c](3073b9cffd24d86269a6767e7e1abef58506d804))
- Address PR review findings ([79a08be](79a08be10ad2d868210a03d868cff5f2e09f6ae3))
- Add blocked node state to REQ-EXEC-002 tracking list ([3c74458](3c74458245896d1841c76c0df5ea882466f8809e))
- Integrate performance metrics emission from performance-reviewing.md ([8f6b203](8f6b203566b3ff7aad5160a011f8d2010a5ffeaa))
- Add alignment verification to architecture spec ([a6fd935](a6fd9353b30a91848c13f0b02a7374e33f4deac5))
- Apply review notes (labels, LLM rate limits, semantic stalling, sub-issues, projects, milestones) ([d18cab8](d18cab85050c64b7e834a9b0c70d71014319cf55))
- Address PR review comments ([452c94d](452c94d63882b43f275d353cb79a1d870539357e))

### ✨ Features

- **spec**: Add context pack system and constitutional security layer (#4) ([71b12c0](71b12c00c4614691ffa1576e7d0734b599e78dd2))
- **pipeline**: Workspace skeleton and pipeline domain types ([2168bdf](2168bdfbe61c90b4a3e184acb163521d64f571e4))
- **pipeline**: Introduce workspace skeleton and domain type definitions (#11) ([892d8e6](892d8e64d53a9718a7c5a41bce7d6cc3cd1a2d26))
- **pipeline**: Add pipeline graph model and runtime state types ([b871f2b](b871f2b136d07a02f152a66d0fb4e63ca3d2a5ab))
- **pipeline**: Add pipeline graph model and runtime state types (#12) ([c875836](c8758363cf50e90e708f0506e6d5ec2a80901488))
- **pipeline**: Define github, event-source, template, and audit traits (#13) ([a61270e](a61270e7765fac9be1b9436de5fdb8be29699b35))

### 🐛 Bug Fixes

- Address PR review feedback ([537e4c8](537e4c81cba4a0307b47d09dbaea6dc6317e9a28))
- **pipeline**: Address PR 11 review comments ([561775f](561775fa2ffb5922cdd8971f6f690f40c02af39b))
- **ci**: Allow first-party git sources and suppress unfixable rsa advisory ([97d189d](97d189d8d82d0a40f519d08439db002a860ddb07))
- **ci**: Suppress transitive unmaintained-crate advisories and fix audit config path ([2f709e1](2f709e1a417ec39829ad5c041c6ef93e5a86d208))
- **ci**: Move audit.toml to .cargo/audit.toml and drop --config flag ([0b136f4](0b136f4bd3c28652b42326b79329b4d27dc83b31))
- **pipeline**: Address PR review findings on graph model ([b3af306](b3af306a3d950b72b52c11bbc71079e8eb7c9266))
- **pipeline**: Address PR review findings on github trait definitions ([94b4de2](94b4de2a651281dbc51a3862dedc1f3a8ca976fa))
- **pipeline**: Rename BlobSha to GitObjectSha and add SDK placeholder alias ([1805094](1805094baf755b892e55d89848abb903229f9f23))

### 👷 CI/CD

- Add claude for code review ([f2e7eb4](f2e7eb49f1e054643d2c12cf2bd545bb91594692))
- Fix secrets baseline creation when no baseline file exists ([b5dbf8f](b5dbf8fa9afc24fbde0731a3c330c360e3324430))
- Add detect-secrets baseline (no secrets found in initial scan) ([ed85e81](ed85e817baadf3b16beda21e83d9cfc382516f30))
- Add all the relevant workflows ([4df7619](4df7619b9c1f32b67fa1ad193c577bfb0ec4d1a4))
- Configs for cargo deny, gitcliff and renovate ([a65b20e](a65b20e91011449ff4efb3ad3b97c13cf9dd52cc))
- Add GitHub Actions workflows and project tooling configuration (#10) ([ef5a1e5](ef5a1e5fa8ddafa80fad074fe7b3d2daa0d2d597))

### 📚 Documentation

- **spec**: Integrate domain generalisation and Extension API addendum ([c6bd5ab](c6bd5abeaa21e134571bb34f44073091c666755e))
- **spec**: Evolve pipeline model from fixed linear sequence to configurable directed graph (#5) ([e42e6d6](e42e6d68fd82f637fa1f5abaffa16291ac4c3798))
- **spec**: Address PR review comments ([0c0be6d](0c0be6d24d50825c81ffd289d3cc7fa3fd12fd63))
- **spec**: Add performance metrics emission architecture (#6) ([0a1c71e](0a1c71e5a488eaa4648ea75feb0ae10f0f12c153))
- **spec**: Address PR review findings for alignment verification ([5fddf06](5fddf061859ee8104ed966029d74d6c22ac6fa6a))
- **spec**: Add alignment verification architecture (#7) ([75fee5c](75fee5cfa56e8c5279e45fa3af0c695313149748))
- **spec**: Address PR review round 2 feedback ([9711047](97110473c4fe21ae4c517ffc7cfc4f267ba42257))
- **spec**: Integrate tool scoping, skill crystallisation, and adapter generation architecture (#8) ([939cc0c](939cc0c6cae254078947d67c3f8f56a601d6aa0c))
- **spec**: Apply review notes for labels, LLM rate limiting, sub-issues, milestones, and projects (#9) ([a0966cc](a0966cc9bbea486b5985c071b9dca0ceb1467d2f))
- **pipeline**: Update specs to match graph model review fixes ([7f8ef17](7f8ef17107cc75f47e494d39a8937aaacadf114c))

<!-- generated by git-cliff -->

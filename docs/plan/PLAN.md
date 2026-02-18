# rem-cli
## Concept
RustでTUI（Terminal User Interface）のTODO管理ツールを作ります。コアコンセプトとしては、rem-cliが作成するTODOデータは全てローカルマシンのファイルシステムに保存されるという点です。これは、remが作成したデータが外部に流出せず、ローカルマシンの中で完結することを意味しており、企業や個人が機密情報を含むTODOデータを扱う際に有効です。


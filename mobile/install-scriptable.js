{
  const main = async () => {
    const sourceUrl = "https://raw.githubusercontent.com/tttol/rem-cli/main/mobile/rem-board.js";
    const request = new Request(sourceUrl);
    const source = await request.loadString();
    const fileManager = FileManager.iCloud();
    const targetPath = fileManager.joinPath(fileManager.documentsDirectory(), "rem-board.js");
    fileManager.writeString(targetPath, source);
    const alert = new Alert();
    alert.title = "rem-board installed";
    alert.message = `Saved to ${targetPath}`;
    alert.addAction("OK");
    await alert.present();
  };
  await main();
}

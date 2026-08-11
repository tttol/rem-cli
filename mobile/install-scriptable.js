{
  const SOURCE_URL = "https://raw.githubusercontent.com/tttol/rem-cli/main/mobile/rem-board.js";
  const SCRIPT_NAME = "rem-board.js";

  const main = async () => {
    const request = new Request(SOURCE_URL);
    const source = await request.loadString();
    const fileManager = FileManager.iCloud();
    const targetPath = fileManager.joinPath(fileManager.documentsDirectory(), SCRIPT_NAME);
    fileManager.writeString(targetPath, source);
    const alert = new Alert();
    alert.title = "rem-board installed";
    alert.message = `Saved to ${targetPath}`;
    alert.addAction("OK");
    await alert.present();
  };
  await main();
}

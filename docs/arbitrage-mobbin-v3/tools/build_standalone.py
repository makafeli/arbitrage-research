"""Build the zero-dependency HTML edition from the editable source files."""
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
css = '\n'.join((ROOT / 'assets' / p).read_text() for p in ['foundation.css','mobbin.css'])
(ROOT / 'assets/styles.css').write_text(css)
html = (ROOT / 'index.html').read_text()
html = html.replace('<link rel="stylesheet" href="assets/styles.css">', '<style>\n'+css+'\n</style>')
html = html.replace('<script src="assets/app.js"></script>', '<script>\n'+(ROOT/'assets/app.js').read_text()+'\n</script>')
(ROOT / 'Arbitrage-Research-Mobbin-v3.html').write_text(html)
print('Built', ROOT / 'Arbitrage-Research-Mobbin-v3.html')

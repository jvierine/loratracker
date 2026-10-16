"""Native chooser runs in a separate main-thread process on every platform."""
import json
import sys
try:
    import tkinter as tk
    from tkinter import filedialog
    root = tk.Tk()
    root.withdraw()
    root.attributes('-topmost', True)
    path = filedialog.askdirectory(title='STRATOS data directory', initialdir=sys.argv[1] if len(sys.argv)>1 else None, mustexist=True, parent=root)
    root.destroy()
    print(json.dumps({'path': path or None}))
except Exception as error:
    print(f'Native folder chooser unavailable: {error}. Enter the directory path instead.', file=sys.stderr)
    sys.exit(1)

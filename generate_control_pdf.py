# generate_control_pdf.py
# Simple script that reads the control_point.md (generated in the previous answer) and writes a PDF.
# It uses the pure‑Python library `reportlab` which is available in the default environment.
# The PDF will contain the plain text of the markdown – no fancy styling, but it serves as a
# point‑of‑control document.

from pathlib import Path
from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import LETTER
from reportlab.lib.units import inch

MARKDOWN_PATH = Path(__file__).with_name('control_point.md')
PDF_PATH = Path(__file__).with_name('control_point.pdf')

if not MARKDOWN_PATH.exists():
    raise FileNotFoundError(f"{MARKDOWN_PATH} not found – create the markdown first.")

# Read the markdown as plain text
text = MARKDOWN_PATH.read_text(encoding='utf-8')

c = canvas.Canvas(str(PDF_PATH), pagesize=LETTER)
width, height = LETTER
margin = inch * 0.75
x = margin
y = height - margin
line_height = 12  # points

for line in text.splitlines():
    # If we run out of space, start a new page
    if y < margin:
        c.showPage()
        y = height - margin
    c.drawString(x, y, line)
    y -= line_height

c.save()
print(f"PDF written to {PDF_PATH}")

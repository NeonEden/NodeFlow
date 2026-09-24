import os
import sys
from pathlib import Path
from typing import List

# Ensure reportlab is available
try:
    from reportlab.lib.pagesizes import LETTER
    from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak
    from reportlab.lib.styles import getSampleStyleSheet
except ImportError:
    import subprocess, sys
    subprocess.check_call([sys.executable, '-m', 'pip', 'install', 'reportlab'])
    from reportlab.lib.pagesizes import LETTER
    from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak
    from reportlab.lib.styles import getSampleStyleSheet


def md_to_paragraphs(text: str) -> List[Paragraph]:
    """Very naive conversion: split by lines, treat headings and normal lines."""
    styles = getSampleStyleSheet()
    flow = []
    for line in text.splitlines():
        line = line.rstrip()
        if not line:
            flow.append(Spacer(1, 12))
            continue
        # headings
        if line.startswith('#'):
            level = len(line) - len(line.lstrip('#'))
            heading_style = styles['Heading%d' % min(level, 3)]
            flow.append(Paragraph(line.lstrip('#').strip(), heading_style))
        else:
            flow.append(Paragraph(line, styles['Normal']))
    return flow


def main():
    base = Path('C:/Users/tomas/Desktop/NodeFlow/nodeflow-desktop/docs')
    out_pdf = base / 'NodeFlow_Documentation.pdf'
    doc = SimpleDocTemplate(str(out_pdf), pagesize=LETTER)
    story = []
    for md_file in sorted(base.glob('*.md')):
        with open(md_file, 'r', encoding='utf-8') as f:
            md_text = f.read()
        story.append(Paragraph(f'<b>{md_file.name}</b>', getSampleStyleSheet()['Title']))
        story.append(Spacer(1, 12))
        story.extend(md_to_paragraphs(md_text))
        story.append(PageBreak())
    doc.build(story)
    print(f'PDF generated at {out_pdf}')

if __name__ == '__main__':
    main()

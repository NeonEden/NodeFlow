import os
from pathlib import Path
from reportlab.lib.pagesizes import LETTER
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer
from reportlab.lib.styles import getSampleStyleSheet

def md_to_flow(text):
    """Convert minimal Markdown to ReportLab flowables."""
    styles = getSampleStyleSheet()
    flow = []
    for line in text.splitlines():
        line = line.rstrip()
        if not line:
            flow.append(Spacer(1, 12))
            continue
        if line.startswith('#'):
            level = len(line) - len(line.lstrip('#'))
            heading_style = styles.get(f'Heading{min(level,3)}', styles['Normal'])
            flow.append(Paragraph(line.lstrip('#').strip(), heading_style))
        else:
            flow.append(Paragraph(line, styles['Normal']))
    return flow

def main():
    base = Path('C:/Users/tomas/Desktop/NodeFlow/nodeflow-desktop/docs')
    src = base / 'SUMMARY.md'
    out = base / 'NodeFlow_Summary.pdf'
    doc = SimpleDocTemplate(str(out), pagesize=LETTER)
    story = []
    with open(src, encoding='utf-8') as f:
        story.extend(md_to_flow(f.read()))
    doc.build(story)
    print(f'Summary PDF generated at {out}')

if __name__ == '__main__':
    main()

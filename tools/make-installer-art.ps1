# Creates agent/src-tauri/installer/sidebar.bmp (164x314) and header.bmp (150x57) for the NSIS installer:
# a dark base with the "aurora" glow of the Glass look, the app icon on a frosted tile and the name.
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "agent\src-tauri\installer"
$icon = [System.Drawing.Image]::FromFile((Join-Path $root "agent\src-tauri\icons\128x128.png"))

function Glow($g, $cx, $cy, $r, $html, $alpha) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $path.AddEllipse($cx - $r, $cy - $r, 2 * $r, 2 * $r)
  $b = New-Object System.Drawing.Drawing2D.PathGradientBrush($path)
  $c = [System.Drawing.ColorTranslator]::FromHtml($html)
  $b.CenterColor = [System.Drawing.Color]::FromArgb($alpha, $c)
  $b.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $c))
  $g.FillPath($b, $path)
}
function Tile($g, $x, $y, $s) {
  $p = New-Object System.Drawing.Drawing2D.GraphicsPath
  $r = $s * 0.28
  $p.AddArc($x, $y, $r, $r, 180, 90); $p.AddArc($x + $s - $r, $y, $r, $r, 270, 90)
  $p.AddArc($x + $s - $r, $y + $s - $r, $r, $r, 0, 90); $p.AddArc($x, $y + $s - $r, $r, $r, 90, 90); $p.CloseFigure()
  $g.FillPath((New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(150, 20, 20, 28))), $p)
  $g.DrawPath((New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(90, 255, 255, 255), 1)), $p)
  $g.DrawImage($icon, $x + $s * 0.12, $y + $s * 0.12, $s * 0.76, $s * 0.76)
}
function Canvas($w, $h) {
  $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = "AntiAlias"; $g.InterpolationMode = "HighQualityBicubic"; $g.TextRenderingHint = "AntiAliasGridFit"
  $g.Clear([System.Drawing.ColorTranslator]::FromHtml("#0b0b0f"))
  return @($bmp, $g)
}

# Sidebar 164x314
$bmp, $g = Canvas 164 314
Glow $g 20 40 150 "#c8461a" 230
Glow $g 150 120 130 "#c8186a" 200
Glow $g 80 320 170 "#5a22b0" 230
Tile $g 42 60 80
$f = New-Object System.Drawing.Font("Segoe UI", 20, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$sf = New-Object System.Drawing.StringFormat; $sf.Alignment = "Center"
$g.DrawString("Decka", $f, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF(0, 156, 164, 30)), $sf)
$f2 = New-Object System.Drawing.Font("Segoe UI", 11, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString("Phone deck for your PC", $f2, (New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(200, 255, 255, 255))), (New-Object System.Drawing.RectangleF(0, 186, 164, 20)), $sf)
$bmp.Save((Join-Path $out "sidebar.bmp"), [System.Drawing.Imaging.ImageFormat]::Bmp); $g.Dispose(); $bmp.Dispose()

# Header 150x57
$bmp, $g = Canvas 150 57
Glow $g 10 10 90 "#c8461a" 220
Glow $g 140 50 80 "#5a22b0" 230
Tile $g 96 8 41
$bmp.Save((Join-Path $out "header.bmp"), [System.Drawing.Imaging.ImageFormat]::Bmp); $g.Dispose(); $bmp.Dispose()
Write-Host "installer art written to $out"

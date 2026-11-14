# Creates tools/icon-source.png: 1024x1024, background #0b0b0f, a white "D" (Segoe UI Bold, 640 px) in the middle.
Add-Type -AssemblyName System.Drawing

$size = 1024
$bitmap = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bitmap)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$g.Clear([System.Drawing.ColorTranslator]::FromHtml("#0b0b0f"))

# 640 px glyph height: GraphicsUnit.Pixel makes the font size a pixel size.
$font = New-Object System.Drawing.Font("Segoe UI", 640, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$format = New-Object System.Drawing.StringFormat
$format.Alignment = [System.Drawing.StringAlignment]::Center
$format.LineAlignment = [System.Drawing.StringAlignment]::Center
$rect = New-Object System.Drawing.RectangleF(0, 0, $size, $size)
$g.DrawString("D", $font, [System.Drawing.Brushes]::White, $rect, $format)

$out = Join-Path $PSScriptRoot "icon-source.png"
$bitmap.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bitmap.Dispose()
Write-Output $out

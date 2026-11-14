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

# Web app icons for the client (client/public), scaled down from the same bitmap.
$public = Join-Path $PSScriptRoot "..\client\public"
New-Item -ItemType Directory -Force $public | Out-Null
foreach ($small in 192, 512) {
    $scaled = New-Object System.Drawing.Bitmap($small, $small)
    $sg = [System.Drawing.Graphics]::FromImage($scaled)
    $sg.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $sg.DrawImage($bitmap, 0, 0, $small, $small)
    $sg.Dispose()
    $scaled.Save((Join-Path $public "icon-$small.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $scaled.Dispose()
}
$bitmap.Dispose()
Write-Output $out

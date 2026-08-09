param(
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$outputRoot = Join-Path $repoRoot 'src\web\apps\web-antd\public\assets\fustfs-baseline'
$baselineMapPath = Join-Path $repoRoot '.agents\skills\baseline-driven-ui\references\baseline-pages.json'
$baselineMap = Get-Content -Raw -Encoding utf8 $baselineMapPath | ConvertFrom-Json

function Resolve-Baseline([string]$key) {
  $entry = $baselineMap.pages | Where-Object { $_.key -eq $key }
  if ($null -eq $entry) {
    throw "Missing baseline map entry: $key"
  }
  return Join-Path $repoRoot ($entry.baseline -replace '/', '\')
}

$assets = @(
  @{
    Source = Resolve-Baseline 'A-04-packed-detail'
    Output = Join-Path $outputRoot 'a04-packed-shield-v1.png'
    Crop = [System.Drawing.Rectangle]::new(539, 648, 64, 60)
    Tone = 'Blue'
  },
  @{
    Source = Resolve-Baseline 'A-04-failed-detail'
    Output = Join-Path $outputRoot 'a04-failed-lock-v1.png'
    Crop = [System.Drawing.Rectangle]::new(539, 583, 50, 60)
    Tone = 'Red'
  },
  @{
    Source = Resolve-Baseline 'A-04-failed-detail'
    Output = Join-Path $outputRoot 'a04-failed-lock-small-v1.png'
    Crop = [System.Drawing.Rectangle]::new(1539, 588, 48, 54)
    Tone = 'Red'
  }
)

New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null

foreach ($asset in $assets) {
  $outputPath = [string]$asset.Output
  if ((Test-Path -LiteralPath $outputPath) -and -not $Force) {
    throw "Output already exists: $outputPath. Use -Force to replace it."
  }

  $sourcePath = (Resolve-Path -LiteralPath ([string]$asset.Source)).Path
  $source = [System.Drawing.Bitmap]::new([string]$sourcePath)
  $cropped = $null
  try {
    $cropped = $source.Clone(
      $asset.Crop,
      [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )

    for ($y = 0; $y -lt $cropped.Height; $y++) {
      for ($x = 0; $x -lt $cropped.Width; $x++) {
        $pixel = $cropped.GetPixel($x, $y)
        $visible = if ($asset.Tone -eq 'Blue') {
          $pixel.B -gt 38 -and
          $pixel.G -gt 30 -and
          $pixel.B -gt ($pixel.R * 1.25) -and
          $pixel.G -gt ($pixel.R * 1.12)
        } else {
          $pixel.R -gt 38 -and
          $pixel.R -gt ($pixel.G * 1.25) -and
          $pixel.R -gt ($pixel.B * 1.12)
        }

        if (-not $visible) {
          $cropped.SetPixel($x, $y, [System.Drawing.Color]::Transparent)
          continue
        }

        $signal = if ($asset.Tone -eq 'Blue') {
          [Math]::Max($pixel.B, $pixel.G)
        } else {
          $pixel.R
        }
        $alpha = [Math]::Min(255, [Math]::Max(0, ($signal - 20) * 3))
        $cropped.SetPixel(
          $x,
          $y,
          [System.Drawing.Color]::FromArgb(
            $alpha,
            $pixel.R,
            $pixel.G,
            $pixel.B
          )
        )
      }
    }

    $cropped.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Output $outputPath
  } finally {
    if ($null -ne $cropped) {
      $cropped.Dispose()
    }
    $source.Dispose()
  }
}

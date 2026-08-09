param(
  [Parameter(Mandatory = $true)]
  [string]$Baseline,

  [Parameter(Mandatory = $true)]
  [string]$Actual,

  [Parameter(Mandatory = $true)]
  [string]$OutputDirectory,

  [Parameter(Mandatory = $true)]
  [string]$Key,

  [ValidateRange(0, 255)]
  [int]$ChannelTolerance = 16
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

function New-CanonicalBitmap {
  param([System.Drawing.Image]$Source)

  $bitmap = [System.Drawing.Bitmap]::new(
    $Source.Width,
    $Source.Height,
    [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
  )
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.DrawImageUnscaled($Source, 0, 0)
  }
  finally {
    $graphics.Dispose()
  }
  return $bitmap
}

$baselinePath = (Resolve-Path -LiteralPath $Baseline).Path
$actualPath = (Resolve-Path -LiteralPath $Actual).Path
$outputRoot = [System.IO.Path]::GetFullPath(
  [System.IO.Path]::Combine((Get-Location).Path, $OutputDirectory)
)
[System.IO.Directory]::CreateDirectory($outputRoot) | Out-Null

$baselineImage = [System.Drawing.Image]::FromFile($baselinePath)
$actualImage = [System.Drawing.Image]::FromFile($actualPath)

try {
  if (
    $baselineImage.Width -ne $actualImage.Width -or
    $baselineImage.Height -ne $actualImage.Height
  ) {
    throw "Image dimensions differ: baseline=$($baselineImage.Width)x$($baselineImage.Height), actual=$($actualImage.Width)x$($actualImage.Height)"
  }

  $width = $baselineImage.Width
  $height = $baselineImage.Height
  $rectangle = [System.Drawing.Rectangle]::new(0, 0, $width, $height)
  $baselineBitmap = New-CanonicalBitmap -Source $baselineImage
  $actualBitmap = New-CanonicalBitmap -Source $actualImage
  $diffBitmap = [System.Drawing.Bitmap]::new(
    $width,
    $height,
    [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
  )

  try {
    $baselineData = $baselineBitmap.LockBits(
      $rectangle,
      [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
      [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )
    $actualData = $actualBitmap.LockBits(
      $rectangle,
      [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
      [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )
    $diffData = $diffBitmap.LockBits(
      $rectangle,
      [System.Drawing.Imaging.ImageLockMode]::WriteOnly,
      [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )

    try {
      $byteCount = [Math]::Abs($baselineData.Stride) * $height
      $baselineBytes = [byte[]]::new($byteCount)
      $actualBytes = [byte[]]::new($byteCount)
      $diffBytes = [byte[]]::new($byteCount)
      [System.Runtime.InteropServices.Marshal]::Copy(
        $baselineData.Scan0,
        $baselineBytes,
        0,
        $byteCount
      )
      [System.Runtime.InteropServices.Marshal]::Copy(
        $actualData.Scan0,
        $actualBytes,
        0,
        $byteCount
      )

      [long]$changedPixels = 0
      [long]$absoluteChannelDifference = 0
      for ($offset = 0; $offset -lt $byteCount; $offset += 4) {
        $blueDifference = [Math]::Abs(
          [int]$baselineBytes[$offset] - [int]$actualBytes[$offset]
        )
        $greenDifference = [Math]::Abs(
          [int]$baselineBytes[$offset + 1] - [int]$actualBytes[$offset + 1]
        )
        $redDifference = [Math]::Abs(
          [int]$baselineBytes[$offset + 2] - [int]$actualBytes[$offset + 2]
        )
        $maximumDifference = [Math]::Max(
          $blueDifference,
          [Math]::Max($greenDifference, $redDifference)
        )
        $absoluteChannelDifference += (
          $blueDifference + $greenDifference + $redDifference
        )
        if ($maximumDifference -gt $ChannelTolerance) {
          $changedPixels++
        }

        $diffBytes[$offset] = [byte][Math]::Min(255, $blueDifference * 4)
        $diffBytes[$offset + 1] = [byte][Math]::Min(
          255,
          $greenDifference * 4
        )
        $diffBytes[$offset + 2] = [byte][Math]::Min(
          255,
          $redDifference * 4
        )
        $diffBytes[$offset + 3] = 255
      }

      [System.Runtime.InteropServices.Marshal]::Copy(
        $diffBytes,
        0,
        $diffData.Scan0,
        $byteCount
      )
    }
    finally {
      $baselineBitmap.UnlockBits($baselineData)
      $actualBitmap.UnlockBits($actualData)
      $diffBitmap.UnlockBits($diffData)
    }

    $diffPath = [System.IO.Path]::Combine($outputRoot, "$Key-diff.png")
    $diffBitmap.Save($diffPath, [System.Drawing.Imaging.ImageFormat]::Png)

    $overlayBitmap = [System.Drawing.Bitmap]::new(
      $width,
      $height,
      [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
    )
    $overlayGraphics = [System.Drawing.Graphics]::FromImage($overlayBitmap)
    $imageAttributes = [System.Drawing.Imaging.ImageAttributes]::new()
    try {
      $overlayGraphics.DrawImageUnscaled($baselineBitmap, 0, 0)
      $colorMatrix = [System.Drawing.Imaging.ColorMatrix]::new()
      $colorMatrix.Matrix33 = 0.5
      $imageAttributes.SetColorMatrix($colorMatrix)
      $overlayGraphics.DrawImage(
        $actualBitmap,
        $rectangle,
        0,
        0,
        $width,
        $height,
        [System.Drawing.GraphicsUnit]::Pixel,
        $imageAttributes
      )
    }
    finally {
      $imageAttributes.Dispose()
      $overlayGraphics.Dispose()
    }

    try {
      $overlayPath = [System.IO.Path]::Combine($outputRoot, "$Key-overlay.png")
      $overlayBitmap.Save(
        $overlayPath,
        [System.Drawing.Imaging.ImageFormat]::Png
      )
    }
    finally {
      $overlayBitmap.Dispose()
    }

    $pixelCount = [long]$width * [long]$height
    $metrics = [ordered]@{
      key = $Key
      baseline = $baselinePath
      actual = $actualPath
      width = $width
      height = $height
      channel_tolerance = $ChannelTolerance
      changed_pixels = $changedPixels
      changed_pixel_ratio = [Math]::Round($changedPixels / $pixelCount, 6)
      mean_absolute_channel_difference = [Math]::Round(
        $absoluteChannelDifference / ($pixelCount * 3),
        4
      )
      normalized_absolute_difference = [Math]::Round(
        $absoluteChannelDifference / ($pixelCount * 3 * 255),
        6
      )
      overlay = $overlayPath
      diff = $diffPath
    }
    $metricsPath = [System.IO.Path]::Combine($outputRoot, "$Key-metrics.json")
    $metrics | ConvertTo-Json | Set-Content -LiteralPath $metricsPath -Encoding utf8
    $metrics | ConvertTo-Json
  }
  finally {
    $diffBitmap.Dispose()
    $actualBitmap.Dispose()
    $baselineBitmap.Dispose()
  }
}
finally {
  $actualImage.Dispose()
  $baselineImage.Dispose()
}

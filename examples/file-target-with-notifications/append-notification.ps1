param()

if ($args.Count -ne 1) {
    [Console]::Error.WriteLine("usage: $PSCommandPath C:\\path\\to\\log.jsonl")
    exit 2
}

$payload = [Console]::In.ReadToEnd()
[System.IO.File]::AppendAllText($args[0], $payload)

package main

import (
	"os"

	"github.com/scriptedworld/bolt/internal/cli"
)

func main() {
	os.Exit(cli.Main(os.Args[1:], os.Stdout, os.Stderr))
}

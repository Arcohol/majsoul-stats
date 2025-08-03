{
  projectRootFile = "flake.nix";
  programs.nixfmt = {
    enable = true;
    strict = true;
  };
  programs.rustfmt.enable = true;
}

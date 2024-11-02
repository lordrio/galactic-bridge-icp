dfx deploy ledger --mode reinstall --argument "
(variant {
Init = record {
    token_name = \"ICP Solana\";
    token_symbol = \"gSol\";
    decimals = opt 9;
    minting_account = record {
    owner = principal \"$(dfx canister id minter)\";
    };
    initial_balances = vec {};
    metadata = vec {};
    transfer_fee = 0;
    archive_options = record {
    trigger_threshold = 2000;
    num_blocks_to_archive = 1000;
    controller_id = principal \"p5mpn-lgd4x-logst-tftlf-ocwbj-b53ah-5mgue-b665z-lv63u-mmvdj-iqe\";
    };
    feature_flags = opt record {
    icrc2 = true;
    };
}
})
"
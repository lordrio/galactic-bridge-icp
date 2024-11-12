dfx deploy minter --mode reinstall --argument "
    (variant {
    Init = record {
        solana_rpc_url = \"https://api.devnet.solana.com\";
        solana_contract_address = \"Cb3CDEGAzbSciL6mb5NAc4pn45k6QyAae9S4agyfEJ2F\";
        solana_initial_signature = \"33rkZhSVeVVfhnKZMdK7tGT2XDo3zW2KAKi89sQvU1g6ChqjWuy8wxwn9oXM5ie5po9ZetQsmXV7nkAKCbgmWUxV\";
        ecdsa_key_name = \"key_1\";
        minimum_withdrawal_amount = 20_000_000;
    }
})
" --ic
"""Guard extracted Kotodama v1 fixtures against their Rust-source preimage."""

from __future__ import annotations

import hashlib
import re
import unittest
from dataclasses import dataclass, replace
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class GuardFailure(AssertionError):
    """Raised when a fixture or its Rust projection drifts."""


@dataclass(frozen=True)
class AssetSpec:
    """Expected byte projection for one versioned Kotodama fixture."""

    name: str
    digest: str
    size: int
    sentinel: bool


@dataclass(frozen=True)
class SourceSpec:
    """Preimage fingerprint and ordered assets for one Rust source."""

    path: str
    skeleton_digest: str
    assets: tuple[AssetSpec, ...]


SOURCES = (
    SourceSpec(
        'crates/ivm/tests/kotodama.rs',
        'a617f933ee35463d73183749471484fe1250406cde7b3bcc9c4c8784da2db3b6',
        (
            AssetSpec('001.ko', '2cd0d5b786b303f342a13799a5828a17903c4b3934b6b7332e994e080039d6ee', 225, True),
            AssetSpec('002.ko', '4b32085d63144de5a6d19634490994a0fadca60737bab8dfb5ef361d1ebd7135', 311, True),
            AssetSpec('003.ko', 'b14baa9b8e0440810a7dce52d52b5f36e792970f24ff2919ddb918708360ce4c', 163, True),
            AssetSpec('004.ko', 'f382e02e14ed9425efacc2ad5aa3982295b108f808578700db2addebdcf68482', 170, True),
            AssetSpec('005.ko', '6acd0f9145754daf4ee414404ec42bb5f911bb15d76b1fc63a9440c6c5c82435', 165, True),
            AssetSpec('006.ko', '80f14302dcc42236cd45cbe6df02c6d8c47a6093ac439c0f5cd09c0ee9d0f145', 206, True),
            AssetSpec('007.ko', '703941afbfd1814013131d4994b68e18db03116902b045638a448eb24025c76d', 784, True),
            AssetSpec('008.ko', '285f3d0cee63b9376b82f7d80a07bd1ed966725a9ede1fed7f20dc3ea6630cc4', 498, True),
            AssetSpec('009.ko', 'b9bb5a002b0ec67b259cc643efbfa231c92f000820b39e597c6b497a82c1dbda', 182, True),
            AssetSpec('010.ko', '993f32bca29416fec872303bf59624c871a3247c4c4de7e3511e6b51c8a3193c', 242, True),
            AssetSpec('011.ko', '8dafec1369197bae80c4c6e9e5f5d20ae97e29b79fe85d7ef6ed94b244a4448e', 236, True),
            AssetSpec('012.ko', '20b74f016c6bb60d3c1ebbceb858bb071bc1b9add55320e9d7153e51bb52fda9', 517, True),
            AssetSpec('013.ko', '48d35b21e5f51653be0d296aa6aab87466e7d739c773871d52ae607d258afba4', 112, True),
            AssetSpec('014.ko', '602d184a1de110a5aeb5c46c4e1667b73aa08dcfcf117acaa54dcd4d7dcb42db', 162, True),
            AssetSpec('015.ko', '91ba073dbd11adc3296b949003de9404d8d96fb9016cc88bf876e375b94fcec2', 160, True),
            AssetSpec('016.ko', '811fc7a898f67069ff71d8903532aeb03bafd0c685962efa0b722dc636e9c073', 166, True),
            AssetSpec('017.ko', '94ed7125f30bd27b2669808da547ce3672274ffb0f75d7bde9a8634554c1920e', 160, True),
            AssetSpec('018.ko', '2578ccf838a45a667ee0c2006624dc52d7919c9218e40f6348aeb668d4144595', 165, True),
            AssetSpec('019.ko', '9cc9765ae187377e3bc6e789af439e24a6484bccd0447552a78274a8f88a1860', 200, True),
            AssetSpec('020.ko', 'd16a70b192a570167675d60fb7d1e819196ddff22287d20ac57c233dd9be88ba', 169, True),
            AssetSpec('021.ko', '77d91765870ff02dc5f237d026ced940fea0a18bd2b5df4703ac8db1818e87d8', 129, True),
            AssetSpec('022.ko', '19848e7e3e9a9d0ed22d36f96ba27667da79be8dd6154700dd4bc234e9bc1e42', 233, True),
            AssetSpec('023.ko', 'e258a52c5903476cb548f0540c0d10048f8bcf8530e58f6fd7c8df035ad00bdc', 377, True),
            AssetSpec('024.ko', '42dc30b6781a7a2627c150a2060683ae7fca5b7ee7eb6e9b535723abc47b0887', 154, True),
            AssetSpec('025.ko', '078be32ef21983f426eb76592942be102f9a82c374e02c0903fa7289e8d2e6e9', 730, True),
            AssetSpec('026.ko', '3a2fe5bf47b042887d4c498308c65833cccfd064556eb7595885e44025c65276', 289, True),
            AssetSpec('027.ko', '559e1abc62ccad8e703e8979bc13e03dd5da16715480cde2f755370cbd979979', 425, True),
            AssetSpec('028.ko', 'b9ba0f19e9bc976a08a90746f5308bf6f7c3785f650943a9c86f2b3adf505d19', 307, True),
            AssetSpec('029.ko', 'b5fed2a934b7531e36aee4926adfa6d138429bff91fede7c59222ca308d30b1f', 178, True),
            AssetSpec('030.ko', '0b659d2a00a6a674c0b1cf37a0e253f4d4ebdb2c07e61f8ef0695d693a94815a', 178, True),
            AssetSpec('031.ko', 'a737212afed59fe5a4dfb9ba3770364599efb525330e709bece80dd6d2755617', 193, True),
            AssetSpec('032.ko', '3bdeef77f6b45fe6e90498f2aea766bc10f07a5cfc9f649d0dd92d7e240a9b6c', 607, True),
            AssetSpec('033.ko', '392f3dcbdc94c293fb1e2deed6d8dfd5bfb2969b9bf70c39ba09beed5bc2aab0', 342, True),
            AssetSpec('034.ko', '6d3e76782c07431125da993926e0d0d716b9de7214e3378a118fa16b27ac6fff', 308, True),
            AssetSpec('035.ko', '3da0613849606b1cf5695ad53a441f6425a86868fe04f399df232749e49d0a52', 130, True),
            AssetSpec('036.ko', '521591eb5771f0bba533ca257b4c2721c8773edbd37eb93d22a881542476e4be', 97, True),
            AssetSpec('037.ko', '149227de4157ea5b63dc146ff495df307f3b35addfcd68a124156067f6f93c4a', 97, True),
            AssetSpec('038.ko', '129f790605313263d6188bc984f3ec21af87cbe24353923e3388ccf767b77c58', 135, True),
            AssetSpec('039.ko', '059a6be55eec0f878e6c5a34bd262d5431498e479b40a6d54661ac027cdc6825', 99, True),
            AssetSpec('040.ko', '1d98703152432503cd3a31f37fca9fbdc63ae578a1cb8eecaba71be843d6133e', 143, True),
            AssetSpec('041.ko', '62425d3edb9ebeddd582a2ddc8886552f826c138218d1bdf69cfa864aa988a9d', 96, True),
            AssetSpec('042.ko', '95b1ed8b5d96addf0d4adec3a9e1abcdc4bda6160655e38ee33245fbfb5beb1c', 134, True),
            AssetSpec('043.ko', '8f81e647161ae8d6c2d175b6fc8a3b4d9ebd2cef03770be7417c9a1cee48398b', 156, True),
            AssetSpec('044.ko', '393bd41567ee72e67bfc7f6d71b13d13d7c9569dfba5fb0c2b5abd5df6b172c2', 180, True),
            AssetSpec('045.ko', 'dbf13f8f1f50e7824695beaeef9b5e5753000982331e2dbbeeb43e89c0999d7f', 189, True),
            AssetSpec('046.ko', 'c0646b2148f2012fb28efccb8d61abb89514a2fd60f801bfbf86e1fe4a5c092e', 259, True),
            AssetSpec('047.ko', 'ca31f7280dbb78c5dc4d73866be7a0402eb31a080a0ba3e90f7755adf34ab91b', 284, True),
            AssetSpec('048.ko', '665b8ea895465de9a67ac26c97ba4ee9eb90c887cbdec60d13794e0c8e8cfcbf', 457, True),
            AssetSpec('049.ko', '223918a0b6e284376121cd6489ef2ca8f90e373629a3827a03ce31eafb9091f8', 246, True),
            AssetSpec('050.ko', 'd5c96c4f397694fed2d1aa565993cf953a7429c4ffd12711abe3a657ce47f253', 490, True),
            AssetSpec('051.ko', '7fbd0716073fa47cd6e956e83afd7890b3a1ac391c6571fdb8e10ab6f75cc8d9', 422, True),
            AssetSpec('052.ko', '36e35dd4284e1ba226cdf463de3b35c43687ef4876b6b8346e6ead1243f58066', 323, True),
            AssetSpec('053.ko', '17cca58a1f8ae78bf36b731bebbd1aba49ee0b685c2006bd00a7d2b98f06be90', 180, True),
            AssetSpec('054.ko', '5915383b64a74fc05edc49f7f247aca41c5a47379c557285211123de0f0d25d2', 501, True),
            AssetSpec('055.ko', '3d62cd3e23eff24e4c9e0144a690b14b23ff6390700f1aedc1d303a9030f4f8b', 548, True),
            AssetSpec('056.ko', '5b7ea73a0e45362b697ee6501f4598179162f96ce50bd5aa7282da5cfb3fa1e9', 369, True),
            AssetSpec('057.ko', 'c67d51b9cd457287966ca3c9a350a998de9fddc7be88de09ea9afb1f049c9c47', 107, False),
            AssetSpec('058.ko', 'abb41577eab75dc83e7e55b7211ac04a6debad0e0964a4980169d5963f328137', 150, False),
            AssetSpec('059.ko', 'd6d15ab8e440a18a4ac92ab84f1aae13c1072c709c6c25c718a702cf8100d3c5', 162, True),
            AssetSpec('060.ko', '93dff9d9e9d04b8721b77cdc867eab9dba6a73a3972951bc58dbb7acafa7eb80', 251, False),
            AssetSpec('061.ko', '6c2aab5068ea6e85550ceaa20361ae16828a5664eb3c3b90e5b0b45a98ac554b', 181, True),
            AssetSpec('062.ko', 'a83e4caa6d3a9374de21474069e94f18f0bad573fd80f7a967a69930d57c03c0', 184, True),
            AssetSpec('063.ko', '2b8a960fff60716f5cecb3a55b84ac826537109ae6e1cceac8e0379e365fa410', 239, True),
            AssetSpec('064.ko', '0ea8cfbfde95bcb823ce737caee66bdf84462f309e3808e1bfcd6dd53a854715', 466, True),
            AssetSpec('065.ko', 'f39fd28a02a80405e1ada957d6fd05d746a2e9c5c894c2f37d9d3ce76dbb4763', 162, True),
            AssetSpec('066.ko', '658453582064d166ec721b21ee468b4eeb506db38543d5f21fe6e81d4c24142d', 176, True),
            AssetSpec('067.ko', 'a3ef06c323fecd3c4ca014f2cdde27d03823bf8324fe1ac041e77036517d0c5b', 150, True),
            AssetSpec('068.ko', 'ff651181676cea49b7be9b8aebee25084604e94acd62e7aee2d70166b7168552', 125, True),
            AssetSpec('069.ko', '5a4efca7c24949bcac1f0b0b5cab8841da9d40e1aa9a9bdc36d687f5a728ff1f', 220, True),
            AssetSpec('070.ko', 'b1cc15ced97ba921503f39122e09fbdac3c27877eefe1a65f328bc0adf87ac3c', 176, True),
            AssetSpec('071.ko', '9ead8388e83e9f1c2f68bf2a4a2f28682bb3313534298545042d7989aafcd2d2', 274, True),
            AssetSpec('072.ko', '65ca96fb48e1f0853bf297c912d5e64f5eba8250cb0edfa63c3b949c9b23b451', 164, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_state_name_map_runtime.rs',
        'ce7b5caf3cd1017ef7e8e59a5da3ae78aa34484318c753e18e333b518e25d70a',
        (
            AssetSpec('001.ko', 'c0d59dd29744c70955b883227fdd3d48e50bbabe387a71b85a62cc2feea227d3', 269, True),
            AssetSpec('002.ko', '5a82559e5cba3e2df840615c6385f3931602c3e9494bee83f3328c47e442e5ff', 396, True),
            AssetSpec('003.ko', '2c3571b2bed367612c48ea17f51f434949fed07191bfb3b3af44cbfdbfeed8ff', 411, True),
            AssetSpec('004.ko', '769f5974f1a69f84bd4a89f086fdb33b108fc33f9bdd7e11821d92c4b4592168', 370, True),
            AssetSpec('005.ko', '5c3fa4dc8f041f7ca01e286bbd4ee9f16cd0a21f3cec67c6314358a3a58a1de0', 363, True),
            AssetSpec('006.ko', '63cdabe7de8392a3acb3fc5aa633cf0148036e3ab373dd3b809056cb1d1be671', 673, True),
            AssetSpec('007.ko', '6f78caa2f2d5dfac0f3cf9f237001c9de1181699f080135d428113216773b556', 1231, True),
            AssetSpec('008.ko', '9f9e69c99e53dca0310123d8319ed66f3934932f67dad65ed04100a114bbe8d4', 1616, True),
            AssetSpec('009.ko', 'a5b81e37f9623c63b884b8d82c1c1b929b4638642328808666108dda56539605', 1479, True),
            AssetSpec('010.ko', '83e6f4c0ae22baed111845366b1a95497a708bc3b3d15401c583109f666521c9', 1159, True),
            AssetSpec('011.ko', 'e0c6e256744e5bb217ed56bbd6aed29f21b2f2f87ddec176c113a538848ab765', 1553, True),
            AssetSpec('012.ko', 'db922cff3b92f0b245471cb1747a301dc71a8c2a30f9cb47e5409b1001cd14da', 431, True),
            AssetSpec('013.ko', 'e19eb88b7ec56c8b6785b1d0c7f7935b6fc4230bc29f376b2406a5a6cfc02c71', 495, True),
            AssetSpec('014.ko', '2c9a5fbf372e062bf9c6b628a5ae15d26749fb9c78e36d16d072f0b2251e878f', 195, True),
            AssetSpec('015.ko', '6473b188d414f8da053cf1c769c91c173cc6179aaab72c3d4d7c30df0f6db529', 195, True),
            AssetSpec('016.ko', 'ac356bdd991059b0900a545ce8e31a043042f6e9ab3258e072bc00f93d98dea3', 744, True),
            AssetSpec('017.ko', 'd500900564fe0cf2fec084d0fec0121f3a95cbe6c1891e9e0d845e42370db3d5', 1622, True),
            AssetSpec('018.ko', '389b11d70e39084793e2a45c730968e7a1d1917b54435f073b258baf41736bdc', 699, True),
            AssetSpec('019.ko', '2c9a5fbf372e062bf9c6b628a5ae15d26749fb9c78e36d16d072f0b2251e878f', 195, True),
            AssetSpec('020.ko', '61f7a1d7ea1b401b93b5fded4e6f77730b858d6a675b4b051e90b02f30dbd035', 337, True),
            AssetSpec('021.ko', '1bdf1b3435d204b9dafa8346ac92fe5d8aaf2fe6c5e0c89fc4938ac1b1798fe4', 394, True),
            AssetSpec('022.ko', 'd01cbe4a0f944be153d92593d516517494bc50fa4f0385c486acda4fb43fac0f', 221, True),
            AssetSpec('023.ko', '0672f14f34e2b78ef2c18ada1226e5c60fbcfaefe5751e650eb346b7bc4b2edb', 318, True),
            AssetSpec('024.ko', '400531ae3682c7ff736888167667fff58cdf75021ac05bbc17dc0472ea40aa8c', 570, True),
            AssetSpec('025.ko', '5acbdf6d5cdca7c2c311268fdbc7855286a48e064e791159b2e66a5f1ce71433', 396, True),
            AssetSpec('026.ko', '3d6a7bb986b92d8f81cb374cf2ca5287b22556eb5d3c7ace387f1db341a4ba67', 519, True),
            AssetSpec('027.ko', 'f4709bdd4a9ae6e590da41bdb0c44f798cb7d074173973c2f463bfaac9681793', 415, True),
            AssetSpec('028.ko', 'a66cadc6aa090c4622c6af4b2706cb6d165e0428fdace2294bcd547d19cda598', 509, True),
            AssetSpec('029.ko', '7f4306b9be35c6657b1f003e1a888efcce8ee0c6a436476cbd2b76e53578a309', 1237, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_v1_runtime_acceptance.rs',
        '3f439f0267ca186c043fd62575e6af2b0320ef767be4a430bdf726ae57f125f1',
        (
            AssetSpec('001.ko', 'e74a429259169313587e92db6f37a267ec784845a32bb273770952315beead8e', 309, False),
            AssetSpec('002.ko', '03ab49f13ad97b62a823f31fa261ee44841b2dd5ff95c7891fea8a3eaceea302', 566, False),
            AssetSpec('003.ko', 'e4865353cede48373fc29e7d67890c18de428130ae57536daa42eceab9de7a54', 467, False),
            AssetSpec('004.ko', '8fcd373fcd0128886088cd3e7078cdfc857f5b417de2f8071eee710af989f8d4', 1024, False),
            AssetSpec('005.ko', 'f2aeb898a0aba9d5b44396c6d058d2d78a22b00d7551555244abfe9a4be04fc2', 899, False),
            AssetSpec('006.ko', '393d51e87ca4d42ae15888f9ffbace829c1e7f49342ac53fe9a7d56775c6f0b9', 1411, False),
            AssetSpec('007.ko', '94c042d66b09a78d956ba981776bd0bc3ec0d374fdd1f683ae92521a76e33800', 219, False),
            AssetSpec('008.ko', 'ac946aa366514a971bac1a9dbd1b622608256d549b39d154e289445817fce700', 204, False),
            AssetSpec('009.ko', '2611e354702608d6df0bbe27a20c73e0c8dea80a61c09cc066eec78c780b272d', 403, False),
            AssetSpec('010.ko', '41e04413da4f085c280e55e6a61994f8bb612051993333cdfdcf15c252d80ebf', 233, False),
            AssetSpec('011.ko', '96d520e8e059e3131c76bbfd3f7700c3c2490b3ee702f49e770ff8444504015a', 1066, False),
            AssetSpec('012.ko', '0c675d75cce585f838906daa785bfb86723baa63da14db609fb1de5545cdf61e', 259, False),
            AssetSpec('013.ko', '20af20313599fef597d4e6a911b722790a4349833093ab0485cf633f4899e146', 958, False),
            AssetSpec('014.ko', 'ffe7247562f6e28500f81895ea257952ca667fea7d41ae360c1eb328bd5c67e5', 225, False),
            AssetSpec('015.ko', '5feef1cf37b5216389879edee71bbf471741ee2ca57c1aecc91d9987ceee9d25', 812, False),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_lists.rs',
        '3e7b519ceb255f054e1c6fe97cecac9a6f4fd2de7d0dff1e796141ed2c112cc8',
        (
            AssetSpec('001.ko', '3b4501aa4bc185ec588c95dab1cdba69b6af4ec811261c2193817c06762e089b', 551, True),
            AssetSpec('002.ko', '06544f9c17080965311c0537d02d2d8f97b34b20a14183cc08a476fca66da4b8', 412, True),
            AssetSpec('003.ko', '2997eec9d68e16f8cfe0180d3934aba49c9d77753033d2a0293a6d97334101c0', 251, True),
            AssetSpec('004.ko', 'e65cca98c6b412a99c1222ae6518d2c69f33144b9e8f9596c70909e99867930d', 164, True),
            AssetSpec('005.ko', '20a299fc9eb9521dd8bdbabf4d04a6f322a53fce63c9e847df9ed86655441879', 2436, True),
            AssetSpec('006.ko', '302d68471d332d57abdf4168500e2d5b8917bb5211c9c6ac5f1a6583fc3291ec', 299, True),
            AssetSpec('007.ko', 'fc476f04c52324d3da9a03bde5e47d1a2a7dde987e6b2aa87418286260be47a9', 166, True),
            AssetSpec('008.ko', '05e7ea350a091f727b91410fce5a9f281326a73ab7dee78c964c3dac4a1cc421', 2523, True),
            AssetSpec('009.ko', '378ccac3bbbefa217795e304349635f58757d2a9e177d8401f57da42b93add87', 243, True),
            AssetSpec('010.ko', '84bd59d0b0cf59aa69fc2e6eb80ac7fc2b2bb542b0a62157b81ed6f1d3b93994', 251, True),
            AssetSpec('011.ko', 'efb4f83e8ec832983b8975082f5feb9e3cbc40c4ba8bc0a29260977f32fd8fca', 210, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/src/koto_test_driver_tests.rs',
        '1bf013dbc7cf174bbeaff7e9c5ac2c1657eff7e3d870007a9c0172d79d060886',
        (
            AssetSpec('001.ko', '7548b3a38f2c54eb30a4ea614e67ebc342db76efc18fad0366863f4e1518e800', 875, True),
            AssetSpec('002.ko', '63961644f937f1cc2e56f76506f3578fc93067ce0da0da17203855519f13394d', 217, True),
            AssetSpec('003.ko', '37ddb3b719eceeee875a6df34cf7d17fd41b4fa567b4c2c68c72f25bd392c594', 267, True),
            AssetSpec('004.ko', '9fc45b8cb8b97a5fe6838693b63ecc4f1c480c356b94659c836f0488492c8dfa', 187, True),
            AssetSpec('005.ko', 'dfcd055a36c0a68f156e629625d5dd554c7c048f8a16c2db0b072fce7962f4a6', 248, True),
            AssetSpec('006.ko', '080477cee70499044c0d28af4431ff5e2bff1537cc26b90858df8eb9265e1db3', 243, True),
            AssetSpec('007.ko', '0751af63650077193b32850f41db8807a1c1acb3c6c087e613fcd5f59169778c', 164, True),
            AssetSpec('008.ko', '1e8d71c182215071d4038d53fa290ad0238c3c8b6bf25011ada4f37e98392a71', 113, True),
            AssetSpec('009.ko', '87bd96ab4c7f49697fc508ab69842552672ffbc834ea31b95d25da29e949d1ad', 238, True),
            AssetSpec('010.ko', '2d329167787e7676a2aef9f908b5a09ce1e647be4e88674da278d5c91d5a3841', 858, True),
            AssetSpec('011.ko', 'fdfa718e143462e7962aee455cdeaf2b3aa7176d1059e928ffa7954964150899', 983, True),
            AssetSpec('012.ko', '49352af113e294121c402255ea0f8c2ef5aaf73f597283e30fc3b248141fc8bc', 162, True),
            AssetSpec('013.ko', '660fe8fb812ad02a589cfba805e6f7d7dc19d83356643d43615a3ab3459d84d5', 442, True),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/tests/sugar_zero_cost.rs',
        'f32e5cd71e6ebb813853b18501695c7af5a0371373037b100011e6b7a1a89206',
        (
            AssetSpec('001.ko', 'aed6f784b9e77424a2b55d1916c62c964da34bda22dc8f6cac10f216677a7900', 230, True),
            AssetSpec('002.ko', '52c86c590def5f2ada37b4924181f2eb723fe3ac5a85032f2d75701b3c019ef2', 385, True),
            AssetSpec('003.ko', 'b64c889140185ef0f34f11548f3b84bcd4c5d0c27f484351323e5c522017437b', 220, True),
            AssetSpec('004.ko', '528760f60f0c307d198b3eba0f537f42887605a33752e86665b1f99a1674ed9d', 361, True),
            AssetSpec('005.ko', '9e93d2ec93955256286079b20461ea50d81f53f6223960f3fded44346c92463a', 105, True),
            AssetSpec('006.ko', '45e753b1261ecc716c1c0b53bf53f51f501980e00f390ae04e4cdd9f9a090714', 113, True),
            AssetSpec('007.ko', '00aa46a21b649b1e240da0edfa604e5c2af716becc453536160e93113018e987', 176, True),
            AssetSpec('008.ko', '45f8f1e52f8fb73d61dc414b13fd5c29a7986b43ccada415ccff2aba35f86db2', 164, True),
            AssetSpec('009.ko', 'ac0ccb657680a291c48fe13f40b8e06dc342f3723847eda980001ac9edda0965', 213, True),
            AssetSpec('010.ko', 'e3b45ae966875d465a104798d844466d55c0c10bdf6ab007e62cf0161c2ab69e', 278, True),
            AssetSpec('011.ko', '5e47b99c06346a4cb1db45614dd55ea2c21b634a4de81e99f7624b8305b0e28c', 223, True),
            AssetSpec('012.ko', '151e9fcef3565ad99c463d222a291962f68a21604dcbe6b4517d3745e6cf5665', 290, True),
            AssetSpec('013.ko', 'bba63f6e8104bd05d9a229b27744883564dd2c35a11caec5551f970da68e04cd', 339, True),
            AssetSpec('014.ko', 'a2d9c16be24ec8525dc50e6a2ede7df7fa08e0beb9452fdf8ec458169fd63bac', 427, True),
            AssetSpec('015.ko', '580b4497f269ff9bd664bce278243ab97fecf1263dbc6bc9a3727ce378ebec55', 554, True),
            AssetSpec('016.ko', '92d841f7c0f9355d8f92747630ecca3220d955e15152bc00c05bdcceb3b95bb1', 658, True),
            AssetSpec('017.ko', '333db7ccc47420e2dd3fbf7e7fdc7762baf0441bb3711a92c2dc5c16755683d7', 278, True),
            AssetSpec('018.ko', '6bb07589b57ccf62cbfd9b2c26ef999d3e375181d43c07ee20e373640cb5d834', 172, True),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_state_aggregate_literal_runtime.rs',
        '65af9350557a584b846de68ad39e7a464f759b291db5d08227383173f0c86fc6',
        (
            AssetSpec('001.ko', 'dee9b111e29ae18bfa974545eda20c3accf2420bfb2596de4dfce17e38d51cd8', 2978, True),
            AssetSpec('002.ko', 'fa16878159bb06bcc92bab63c9b955c4a15cb71f1571e56e5b095b168851d61b', 1454, True),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/src/compiler/tests/staged_mint_access_hints.rs',
        '79a04d2d38d38cc69f86c6eea381d87ba27a67aed990b92d5e43a49f8c674ccd',
        (
            AssetSpec('001.ko', '84c5f786e83b467f1f9799bfcd79e1c2f42e983d0207ae16bf586c8591b2c571', 3546, False),
        ),
    ),
    SourceSpec(
        'crates/ivm/tests/kotodama_state_scalar.rs',
        'c5ad7f7744504ebbc39643e08fb517caca7050f8a0f6fad4d8e03b19d9e108e5',
        (
            AssetSpec('001.ko', '378b20a6b02c767716a13c52402a2e60f0755bf6d2d3ae1cc26bef854a24fe41', 198, True),
            AssetSpec('002.ko', '88c59c7bfc201457408150dd24125bc13785df86a5ec23ccd9fdffc0b9f179e8', 741, True),
            AssetSpec('003.ko', '580b4497f269ff9bd664bce278243ab97fecf1263dbc6bc9a3727ce378ebec55', 554, True),
            AssetSpec('004.ko', '92d841f7c0f9355d8f92747630ecca3220d955e15152bc00c05bdcceb3b95bb1', 658, True),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/src/resolved.rs',
        'dfeef80a82e2dac369312c85b0aa70709c4d18d1a634b176c505192948852d58',
        (
            AssetSpec('001.ko', 'a15f6256b419624f839af6961586120b08958cb3886b9f7a51671471b7a85e88', 176, False),
            AssetSpec('002.ko', 'fbdce614e48b118614c6817b2ec40eaa3719125c19ec72fe332acd0d5c0e8577', 249, False),
            AssetSpec('003.ko', '1e7b9d779fc7a9874e53d4ddbe32da61318f813a42b04ad816d398f1070d15c7', 191, False),
            AssetSpec('004.ko', '7b0764e31e6fbe2cd284a2fa67564d9ff77273dac2a22a0968ed7685ca76f3e1', 497, False),
            AssetSpec('005.ko', 'eceba390f4dd9d44387db298d606595ee18d6f9b92b4d47f64118c2bbf7df89c', 127, False),
            AssetSpec('006.ko', 'b634a8c1d989eaabc4b6590ddff68e41edb613756c4176e12a9453de2ee88bdd', 345, False),
            AssetSpec('007.ko', '87661418f9030af2bd1e4f97bf1c2eeaddc2189d74aa3a13fffca2ace995a701', 114, False),
            AssetSpec('008.ko', '9551c6c77e2c1208b07c941fb3da0518c6c94319a30f328d251acd83cdc2911f', 191, False),
            AssetSpec('009.ko', 'e9cebaea7200fb9cfbad28f7dc410a2f99089302690f5151201660b7891994f5', 92, False),
            AssetSpec('010.ko', '82f16c53cd025ea983c18e689356356f1e6720605aceff02fe8202b08f1ef0c7', 157, False),
        ),
    ),
    SourceSpec(
        'crates/kotodama_lang/src/secret.rs',
        'd3bec512e608596622a1358926652b1fbf2f191a0ac887823b769fae310aad42',
        (
            AssetSpec('001.ko', '2cabbe93cb0612dc119067e16807af6d72934d1fef6cbf0b193f9fcaea6cc746', 328, True),
            AssetSpec('002.ko', 'cafb2cda4762224b1a8cda1e3eb0c3f7dcb49346185613905a294f9eac69bfe6', 224, True),
            AssetSpec('003.ko', '76ca4ca26761bda42d8b66b85d3d6f0f6a6790c0eb68e8c11f149f2f33152de3', 366, True),
            AssetSpec('004.ko', 'a0ef136bcc6e4951b8259a0d8621214534d97d4d045735641766ddfd02e6f906', 237, True),
            AssetSpec('005.ko', 'c08929360d22340031ba1276a26beb966780b53ca831c5c4d3315dd2d8403e26', 250, True),
            AssetSpec('006.ko', 'a56b35698a91a4a912bdd9a31f36c97d7c44718faddd3fd48f8f974647f490b7', 295, True),
            AssetSpec('007.ko', '139dd62355ac0f230626029115da1530fb86ef1ad164034d6cfaf120298f981d', 289, True),
            AssetSpec('008.ko', '87dae5504c5cb7f3f6c82404442c185f366b25bc83fc00a6c5f788112c57fcea', 364, True),
        ),
    ),
)

_INCLUDE_RE = re.compile(
    rb'include_str!\([ \t\r\n]*"(?P<path>[^"\r\n]*fixtures/koto_v1/[^"\r\n]+)"[ \t\r\n]*\)'
    rb'(?P<sentinel>[ \t\r\n]*\.strip_suffix\([ \t\r\n]*\'\\n\'[ \t\r\n]*\)'
    rb'[ \t\r\n]*\.expect\([ \t\r\n]*"fixture sentinel newline"[ \t\r\n]*\))?'
)


def _asset_repo_path(source: SourceSpec, asset: AssetSpec) -> Path:
    """Return the repository-relative asset path implied by the source."""

    crate = Path(source.path).parts[1]
    return Path("crates") / crate / "fixtures" / "koto_v1" / Path(source.path).stem / asset.name


def _marker(asset_path: Path) -> bytes:
    """Return the canonical placeholder used by the preimage fingerprint."""

    return f'__KOTODAMA_FIXTURE__("{asset_path.as_posix()}")'.encode()


def _project_asset(spec: AssetSpec, stored: bytes) -> bytes:
    """Apply the checked sentinel projection and validate exact bytes."""

    if spec.sentinel:
        if not stored.endswith(b"\n"):
            raise GuardFailure(f"{spec.name}: missing sentinel newline")
        projected = stored[:-1]
    else:
        projected = stored
    if len(projected) != spec.size:
        raise GuardFailure(f"{spec.name}: projected size drift")
    if hashlib.sha256(projected).hexdigest() != spec.digest:
        raise GuardFailure(f"{spec.name}: projected byte digest drift")
    return projected


def _normalize_source(source: SourceSpec, data: bytes) -> bytes:
    """Replace checked include expressions with canonical preimage markers."""

    matches = list(_INCLUDE_RE.finditer(data))
    if len(matches) != len(source.assets):
        raise GuardFailure(f"{source.path}: expected {len(source.assets)} fixture includes, found {len(matches)}")
    chunks: list[bytes] = []
    cursor = 0
    source_dir = (ROOT / source.path).parent
    for match, asset in zip(matches, source.assets):
        expected_repo_path = _asset_repo_path(source, asset)
        included = match.group("path").decode()
        resolved = (source_dir / included).resolve()
        expected = (ROOT / expected_repo_path).resolve()
        if resolved != expected:
            raise GuardFailure(f"{source.path}: include path drift for {asset.name}")
        has_sentinel_projection = match.group("sentinel") is not None
        if has_sentinel_projection != asset.sentinel:
            raise GuardFailure(f"{source.path}: sentinel projection drift for {asset.name}")
        chunks.extend((data[cursor:match.start()], _marker(expected_repo_path)))
        cursor = match.end()
    chunks.append(data[cursor:])
    normalized = b"".join(chunks)
    if hashlib.sha256(normalized).hexdigest() != source.skeleton_digest:
        raise GuardFailure(f"{source.path}: non-fixture Rust preimage drift")
    return normalized


def _validate_checkout() -> None:
    """Validate all sources, assets, projections, and the closed asset set."""

    expected_assets: set[Path] = set()
    for source in SOURCES:
        _normalize_source(source, (ROOT / source.path).read_bytes())
        for asset in source.assets:
            repo_path = _asset_repo_path(source, asset)
            expected_assets.add(repo_path)
            _project_asset(asset, (ROOT / repo_path).read_bytes())
    actual_assets: set[Path] = set()
    for crate in ("ivm", "kotodama_lang"):
        fixture_root = ROOT / "crates" / crate / "fixtures" / "koto_v1"
        if fixture_root.exists():
            actual_assets.update(path.relative_to(ROOT) for path in fixture_root.rglob("*.ko"))
    if actual_assets != expected_assets:
        missing = sorted(expected_assets - actual_assets)
        extra = sorted(actual_assets - expected_assets)
        raise GuardFailure(f"fixture asset set drift; missing={missing}, extra={extra}")


class KotodamaFixtureAssetSourceGuard(unittest.TestCase):
    """Keep extraction semantics and mutation failures explicit."""

    def test_checkout_matches_preimage(self) -> None:
        _validate_checkout()

    def test_payload_mutation_fails_closed(self) -> None:
        source = SOURCES[0]
        spec = source.assets[0]
        stored = (ROOT / _asset_repo_path(source, spec)).read_bytes()
        mutated = bytes((stored[0] ^ 1,)) + stored[1:]
        with self.assertRaises(GuardFailure):
            _project_asset(spec, mutated)

    def test_sentinel_mutation_fails_closed(self) -> None:
        source, spec = next(
            (source, asset)
            for source in SOURCES
            for asset in source.assets
            if asset.sentinel
        )
        stored = (ROOT / _asset_repo_path(source, spec)).read_bytes()
        with self.assertRaises(GuardFailure):
            _project_asset(replace(spec, sentinel=False), stored)

    def test_rust_skeleton_mutation_fails_closed(self) -> None:
        source = SOURCES[0]
        data = (ROOT / source.path).read_bytes()
        mutated = data.replace(b"#[test]", b"#[cfg(test)]", 1)
        with self.assertRaises(GuardFailure):
            _normalize_source(source, mutated)


if __name__ == "__main__":
    unittest.main()
